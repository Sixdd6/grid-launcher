import { describe, expect, it } from 'vitest';
import { driveInfo, packSummary, row } from '../../fixtures.test-util';
import { backupAllLabel, DEFAULT_FOLDS, folderHint, folderKey, folderOf, libraryRail, NO_DRIVES, railEntry, railKeys, railRowKey } from './libraryRail';

const NOW = 1_800_000_000;
const DAY = 24 * 60 * 60;

describe('the TV Library rail (items 176–184, 420)', () => {
  const rows = [
    row({ rom_id: 1, title: 'A', platform: 'SNES', last_played_at: NOW - 2 * DAY }),
    row({ rom_id: 2, title: 'B', platform: 'SNES', last_played_at: NOW - 40 * DAY }),
    row({ rom_id: 3, title: 'C', platform: 'PlayStation', last_played_at: 0 }),
    row({ rom_id: 4, title: 'Emu', platform: 'Emulators' }),
  ];

  it('counts All games, Recent (30 days), Updates and each platform, Emulators left out', () => {
    const rail = libraryRail(rows, new Set([3]), NOW, []);
    expect(rail.filters.map((e) => [e.key, e.label, e.count])).toEqual([
      ['all', 'All games', 3],
      ['recent', 'Recent', 1],
      ['updates', 'Updates', 1],
    ]);
    expect(rail.platforms.map((e) => [e.key, e.label, e.count])).toEqual([
      ['platform:playstation', 'PlayStation', 1],
      ['platform:snes', 'SNES', 2],
    ]);
    expect(rail.drives).toEqual([]);
  });

  it('has one row per connected drive with a pack, with its game count', () => {
    const drives = [driveInfo('/m/t7', { pack: packSummary({ name: 'Couch kit', game_count: 8 }) }), driveInfo('/m/new', { pack: null, status: 'not_set_up' })];
    const rail = libraryRail(rows, new Set(), NOW, drives);
    expect(rail.drives.map((e) => [e.key, e.label, e.count])).toEqual([['drive:/m/t7', 'Couch kit', 8]]);
    expect(railKeys(rail)).toEqual(['all', 'recent', 'updates', 'platform:playstation', 'platform:snes', 'drive:/m/t7']);
    expect(NO_DRIVES).toBe('No Backup drives connected');
  });

  it('falls back to All games for a row that went away', () => {
    const rail = libraryRail(rows, new Set(), NOW, []);
    expect(railEntry(rail, 'drive:/m/gone').key).toBe('all');
    expect(railEntry(rail, 'platform:snes').label).toBe('SNES');
  });

  it('folders start open; the header hint is Close / Open; keys as the prototype', () => {
    expect(DEFAULT_FOLDS).toEqual({ plat: true, drives: true });
    expect(folderHint(true)).toBe('Close');
    expect(folderHint(false)).toBe('Open');
    expect(folderKey('plat')).toBe('fold-plat');
    expect(railRowKey('all')).toBe('r-all');
    expect(folderOf('platform:snes')).toBe('plat');
    expect(folderOf('drive:/m/t7')).toBe('drives');
    expect(folderOf('recent')).toBeNull();
  });

  it('X on a platform row backs up its installed games', () => {
    expect(backupAllLabel(3)).toBe('Backup all 3…');
  });
});
