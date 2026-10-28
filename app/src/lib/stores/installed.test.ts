import { describe, expect, it } from 'vitest';
import type { GameSummary, InstalledGame } from '../api';
import { buildInstalledIndex, indexHas, matchesInstalled } from './installed.svelte';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  return {
    title: 'Chrono Trigger',
    platform: 'SNES',
    rom_id: 42,
    rom_file_name: 'chrono.sfc',
    archive_path: '',
    extracted_path: '',
    extracted_dir: '',
    multi_file_game_dir: '',
    description: '',
    rating: '',
    genres: '',
    regions: '',
    languages: '',
    tags: '',
    revision: '',
    companies: '',
    first_release_date: '',
    filesize_bytes: 0,
    server_updated_at: '',
    installed_at: 0,
    cover_small_path: '',
    cover_large_path: '',
    screenshot_urls: '',
    fanart_urls: '',
    native_executable_path: '',
    native_launch_parameters: '',
    native_compat_tool: '',
    native_wineprefix: '',
    native_game_dir: '',
    included_dlc: '',
    ps3_trophy_paths: '',
    ps3_game_id: '',
    ps3_iso_path: '',
    ps4_game_id: '',
    ps4_content: '',
    ra_id: '',
    last_played_at: 0,
    ...overrides,
  };
}

function game(overrides: Partial<GameSummary>): GameSummary {
  return {
    id: 42,
    name: 'Chrono Trigger',
    platform_id: 1,
    path_cover_small: null,
    path_cover_large: null,
    screenshot_urls: [],
    fanart_urls: [],
    ...overrides,
  };
}

describe('matchesInstalled', () => {
  it('matches on rom_id when it is set, regardless of title/platform text', () => {
    const r = row({ rom_id: 42, title: 'Something Else', platform: 'Other' });
    expect(matchesInstalled(r, game({ id: 42 }), 'SNES')).toBe(true);
  });

  it('does not match a different rom_id when identity also differs', () => {
    const r = row({ rom_id: 42, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 99, name: 'Chrono Cross' }), 'SNES')).toBe(false);
  });

  it('falls back to identity match when rom_id is null', () => {
    const r = row({ rom_id: null, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42 }), 'SNES')).toBe(true);
  });

  it('identity match is case-insensitive', () => {
    const r = row({ rom_id: null, title: 'CHRONO TRIGGER', platform: 'snes' });
    expect(matchesInstalled(r, game({ id: 42, name: 'chrono trigger' }), 'SNES')).toBe(true);
  });

  it('identity match trims surrounding whitespace', () => {
    const r = row({ rom_id: null, title: '  Chrono Trigger  ', platform: ' SNES ' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(true);
  });

  it('does not match when rom_id is null and title differs', () => {
    const r = row({ rom_id: null, title: 'Chrono Cross', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });

  it('does not match when rom_id is null and platform differs', () => {
    const r = row({ rom_id: null, title: 'Chrono Trigger', platform: 'Genesis' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });

  it('does NOT fall back to identity when rom_id is set but mismatched, even if title/platform match', () => {
    // The rom_id comparison is authoritative once the row has one (docs/porting/
    // 03-library-install.md identity rules) — no identity rescue, otherwise a
    // duplicate-title library would badge/uninstall the wrong game.
    const r = row({ rom_id: 7, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });
});

describe('buildInstalledIndex / indexHas', () => {
  // A registry mixing id rows, identity-only rows and near misses: the index
  // must answer exactly what scanning every row with `matchesInstalled` does.
  const rows = [
    row({ rom_id: 42, title: 'Chrono Trigger', platform: 'SNES' }),
    row({ rom_id: 7, title: 'Earthbound', platform: 'SNES' }),
    row({ rom_id: null, title: '  Secret of Mana ', platform: 'snes' }),
    row({ rom_id: null, title: 'Sonic', platform: 'Genesis' }),
  ];
  const games = [
    game({ id: 42, name: 'Chrono Trigger' }),
    game({ id: 99, name: 'Earthbound' }), // title of an id row, other id
    game({ id: 7, name: 'Renamed on server' }),
    game({ id: 5, name: 'secret of mana' }),
    game({ id: 6, name: 'Sonic' }), // identity row on another platform
    game({ id: 8, name: 'Zelda' }),
  ];

  it.each(['SNES', 'Genesis', ' snes '])('agrees with matchesInstalled on %j', (platform) => {
    const index = buildInstalledIndex(rows);
    for (const g of games) {
      const scanned = rows.some((r) => matchesInstalled(r, g, platform));
      expect(indexHas(index, g, platform), `${g.name} on ${platform}`).toBe(scanned);
    }
  });

  it('is empty for an empty registry', () => {
    expect(indexHas(buildInstalledIndex([]), game({}), 'SNES')).toBe(false);
  });
});
