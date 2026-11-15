import { describe, expect, it } from 'vitest';
import { driveInfo, packSummary, row } from '../../fixtures.test-util';
import { DEFAULT_FOLDS, folderHint, folderKey, folderOf, libraryRail, NO_DRIVES, railEntry, railKeys, railRowKey, storefrontGroups } from './libraryRail';

const NOW = 1_800_000_000;
const DAY = 24 * 60 * 60;

describe('the TV Library rail (items 176–184, 420; TVRommStorefront: no Platforms folder)', () => {
  const rows = [
    row({ rom_id: 1, title: 'A', platform: 'SNES', last_played_at: NOW - 2 * DAY }),
    row({ rom_id: 2, title: 'B', platform: 'SNES', last_played_at: NOW - 40 * DAY }),
    row({ rom_id: 3, title: 'C', platform: 'PlayStation', last_played_at: 0 }),
    row({ rom_id: 4, title: 'Emu', platform: 'Emulators' }),
  ];

  it('counts All games, Recent (30 days) and Updates, Emulators left out; no platform rows', () => {
    const rail = libraryRail(rows, new Set([3]), NOW, []);
    expect(rail.filters.map((e) => [e.key, e.label, e.count])).toEqual([
      ['all', 'All games', 3],
      ['recent', 'Recent', 1],
      ['updates', 'Updates', 1],
    ]);
    expect(rail.drives).toEqual([]);
    expect(railKeys(rail).some((k) => k.startsWith('platform:'))).toBe(false);
  });

  it('has one row per connected drive with a pack, with its game count', () => {
    const drives = [driveInfo('/m/t7', { pack: packSummary({ name: 'Couch kit', game_count: 8 }) }), driveInfo('/m/new', { pack: null, status: 'not_set_up' })];
    const rail = libraryRail(rows, new Set(), NOW, drives);
    expect(rail.drives.map((e) => [e.key, e.label, e.count])).toEqual([['drive:/m/t7', 'Couch kit', 8]]);
    expect(railKeys(rail)).toEqual(['all', 'recent', 'updates', 'drive:/m/t7']);
    expect(NO_DRIVES).toBe('No backup drives connected');
  });

  it('falls back to All games for a row that went away', () => {
    const rail = libraryRail(rows, new Set(), NOW, []);
    expect(railEntry(rail, 'drive:/m/gone').key).toBe('all');
    expect(railEntry(rail, 'platform:snes').key).toBe('all');
  });

  it('the drives folder starts open; the header hint is Close / Open; keys as the prototype', () => {
    expect(DEFAULT_FOLDS).toEqual({ drives: true });
    expect(folderHint(true)).toBe('Close');
    expect(folderHint(false)).toBe('Open');
    expect(folderKey('drives')).toBe('fold-drives');
    expect(railRowKey('all')).toBe('r-all');
    expect(folderOf('drive:/m/t7')).toBe('drives');
    expect(folderOf('recent')).toBeNull();
  });
});

describe('the storefront groups (decision 10; TVRommStorefront)', () => {
  const stores = [{ source: 'amazon' }, { source: 'epic' }, { source: 'gog' }, { source: 'steam' }];
  it('puts RomM between GOG and Steam while its group shows', () => {
    expect(storefrontGroups(stores, true).map((g) => (g === 'romm' ? 'romm' : g.source))).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
  });
  it('leaves RomM out when it has no group', () => {
    expect(storefrontGroups(stores, false).map((g) => (g === 'romm' ? 'romm' : g.source))).toEqual(['amazon', 'epic', 'gog', 'steam']);
    expect(storefrontGroups([{ source: 'steam' }, { source: 'gog' }], true).map((g) => (g === 'romm' ? 'romm' : g.source))).toEqual(['gog', 'romm', 'steam']);
  });
});
