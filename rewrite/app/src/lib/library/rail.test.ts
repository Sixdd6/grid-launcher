import { describe, expect, it } from 'vitest';
import type { InstalledGame } from '../api';
import {
  emptyText,
  entryForKey,
  matchesRail,
  platformSlug,
  railEntries,
  RECENT_WINDOW_SECONDS,
} from './rail';

const NOW = 1_800_000_000;

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

describe('platformSlug', () => {
  it('lowercases, trims and joins runs of punctuation with a single dash', () => {
    expect(platformSlug('Super Nintendo Entertainment System')).toBe(
      'super-nintendo-entertainment-system',
    );
    expect(platformSlug('  PlayStation 3 ')).toBe('playstation-3');
    expect(platformSlug('Sega CD / Mega-CD')).toBe('sega-cd-mega-cd');
  });
  it('never yields a leading or trailing dash', () => {
    expect(platformSlug('!Arcade!')).toBe('arcade');
  });
});

describe('railEntries (design section 5)', () => {
  const rows = [
    row({ rom_id: 1, platform: 'SNES', last_played_at: NOW - 100 }),
    row({ rom_id: 2, platform: 'SNES', last_played_at: 0 }),
    row({ rom_id: 3, platform: 'Arcade', last_played_at: NOW - RECENT_WINDOW_SECONDS - 1 }),
    row({ rom_id: 4, platform: 'Emulators', last_played_at: NOW }),
  ];
  const updates = new Set([2, 3]);

  it('lists All games, Recent, Updates, then platforms sorted by name', () => {
    expect(railEntries(rows, updates, NOW).map((e) => e.key)).toEqual([
      'all',
      'recent',
      'updates',
      'platform:arcade',
      'platform:snes',
    ]);
  });

  it('hides the synthetic Emulators platform from the rail and every count', () => {
    const entries = railEntries(rows, updates, NOW);
    expect(entries.map((e) => e.key)).not.toContain('platform:emulators');
    expect(entryForKey(entries, 'all').count).toBe(3);
  });

  it('counts Recent as played inside the 30-day window, never a zero stamp', () => {
    expect(entryForKey(railEntries(rows, updates, NOW), 'recent').count).toBe(1);
  });

  it('counts Updates only for rows the update set names', () => {
    expect(entryForKey(railEntries(rows, updates, NOW), 'updates').count).toBe(2);
  });

  it('counts and labels each platform', () => {
    const snes = entryForKey(railEntries(rows, updates, NOW), 'platform:snes');
    expect(snes.label).toBe('SNES');
    expect(snes.count).toBe(2);
    expect(snes.testId).toBe('library-rail-platform-snes');
  });

  it('gives the three fixed entries their design section 11 ids', () => {
    expect(railEntries(rows, updates, NOW).slice(0, 3).map((e) => e.testId)).toEqual([
      'library-rail-all',
      'library-rail-recent',
      'library-rail-updates',
    ]);
  });

  it('still lists the three fixed entries, at zero, for an empty library', () => {
    expect(railEntries([], new Set(), NOW).map((e) => e.count)).toEqual([0, 0, 0]);
  });
});

describe('matchesRail', () => {
  const played = row({ rom_id: 1, platform: 'SNES', last_played_at: NOW - 10 });
  const stale = row({ rom_id: 2, platform: 'Arcade', last_played_at: 0 });
  const updates = new Set([2]);

  it('accepts everything for All games', () => {
    expect(matchesRail(played, 'all', updates, NOW)).toBe(true);
    expect(matchesRail(stale, 'all', updates, NOW)).toBe(true);
  });
  it('accepts only rows played inside the window for Recent', () => {
    expect(matchesRail(played, 'recent', updates, NOW)).toBe(true);
    expect(matchesRail(stale, 'recent', updates, NOW)).toBe(false);
  });
  it('treats a stamp exactly on the window edge as recent', () => {
    const edge = row({ rom_id: 3, last_played_at: NOW - RECENT_WINDOW_SECONDS });
    expect(matchesRail(edge, 'recent', updates, NOW)).toBe(true);
  });
  it('accepts only rows in the update set for Updates', () => {
    expect(matchesRail(stale, 'updates', updates, NOW)).toBe(true);
    expect(matchesRail(played, 'updates', updates, NOW)).toBe(false);
  });
  it('never matches Updates for a row with no rom id', () => {
    expect(matchesRail(row({ rom_id: null }), 'updates', updates, NOW)).toBe(false);
  });
  it('matches a platform entry case- and space-insensitively', () => {
    expect(matchesRail(row({ platform: ' snes ' }), 'platform:snes', updates, NOW)).toBe(true);
    expect(matchesRail(row({ platform: 'Arcade' }), 'platform:snes', updates, NOW)).toBe(false);
  });
});

describe('emptyText (design section 5, verbatim)', () => {
  it('reads differently for each rail entry', () => {
    expect(emptyText({ key: 'all', testId: 'library-rail-all', label: 'All games', count: 0 })).toBe(
      'No games installed',
    );
    expect(
      emptyText({ key: 'recent', testId: 'library-rail-recent', label: 'Recent', count: 0 }),
    ).toBe('Nothing played in the last 30 days');
    expect(
      emptyText({ key: 'updates', testId: 'library-rail-updates', label: 'Updates', count: 0 }),
    ).toBe('Everything is up to date');
    expect(
      emptyText({
        key: 'platform:snes',
        testId: 'library-rail-platform-snes',
        label: 'SNES',
        count: 0,
      }),
    ).toBe('No games installed for SNES');
  });
});
