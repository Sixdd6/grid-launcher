import { describe, expect, it } from 'vitest';
import { sortLabel } from '../../../library/sort';
import { driveGame, driveInfo, packSummary, row } from '../../fixtures.test-util';
import { countLine, DEFAULT_SORT, driveChips, driveCountLine, driveEmptyState, driveGrid, emptyState, libraryGrid, searching, SEARCH_TITLE, SORT_TITLE, TV_SORTS } from './libraryHeader';

describe('the TV Library header (items 185, 187–191)', () => {
  it('counts games', () => {
    expect(countLine(1)).toBe('1 game');
    expect(countLine(0)).toBe('0 games');
    expect(countLine(12)).toBe('12 games');
  });

  it('sorts by Recently played (default), Recently installed, Title, Platform', () => {
    expect(SORT_TITLE).toBe('Sort by');
    expect(DEFAULT_SORT).toBe('played');
    expect(TV_SORTS.map(sortLabel)).toEqual(['Recently played', 'Recently installed', 'Title', 'Platform']);
    expect(SEARCH_TITLE).toBe('Search your library');
  });

  it('narrows by title; a blank search is no filter', () => {
    const rows = [row({ title: 'Zelda', last_played_at: 5 }), row({ title: 'Mario', last_played_at: 9 }), row({ title: 'Metroid', last_played_at: 0 })];
    expect(libraryGrid(rows, '', 'played').map((r) => r.title)).toEqual(['Mario', 'Zelda', 'Metroid']);
    expect(libraryGrid(rows, '  ', 'title').map((r) => r.title)).toEqual(['Mario', 'Metroid', 'Zelda']);
    expect(libraryGrid(rows, 'm', 'title').map((r) => r.title)).toEqual(['Mario', 'Metroid']);
    expect(searching('  ')).toBe(false);
    expect(searching('z')).toBe(true);
  });

  it('says why the grid is empty', () => {
    expect(emptyState('all', 'zz')).toEqual({ title: 'No games match “zz”', body: 'Press (Y) to change the search.' });
    expect(emptyState('updates', '')).toEqual({ title: 'Everything is up to date', body: 'Games with a newer copy on your server show here.' });
    expect(emptyState('updates', ' ')).toEqual({ title: 'Everything is up to date', body: 'Games with a newer copy on your server show here.' });
    expect(emptyState('recent', '')).toEqual({ title: 'No games here yet', body: 'Install games from the Server tab.' });
    expect(emptyState('platform:snes', '')).toEqual({ title: 'No games here yet', body: 'Install games from the Server tab.' });
  });
});

describe('a Backup drive in the Library (items 421–423)', () => {
  const t7 = driveInfo('/m/t7', {
    pack: packSummary({
      emulators: [
        { name: 'PCSX2', version: '2', os: 'linux' },
        { name: 'PCSX2', version: '2', os: 'windows' },
        { name: 'DuckStation', version: '1', os: 'linux' },
      ],
      firmware: ['PlayStation 2', 'PlayStation'],
    }),
  });

  it('heads with the games shown, their size and the volume label, and the chips', () => {
    expect(driveCountLine(t7, [driveGame({ size: 2_000_000_000 }), driveGame({ size: 1_500_000_000 })])).toMatch(/^2 games · 3(\.\d)? GB · on SAMSUNG T7$/);
    expect(driveCountLine(t7, [driveGame({ size: 0 })])).toMatch(/^1 game · /);
    expect(driveChips(t7)).toEqual({ emulators: 'PCSX2, DuckStation', firmware: '2 systems' });
    expect(driveChips(driveInfo('/m/x', { pack: packSummary({ firmware: ['PS2'] }) }))).toEqual({ emulators: 'none', firmware: '1 system' });
  });

  it('sorts and searches the drive games, played and installed by this computer\'s copy', () => {
    const games = [driveGame({ rom_id: 1, name: 'Bravo', platform_name: 'PS2' }), driveGame({ rom_id: 2, name: 'Alpha', platform_name: 'PSX' }), driveGame({ rom_id: 3, name: 'Charlie', platform_name: 'GC' })];
    const rows = new Map([[3, row({ rom_id: 3, last_played_at: 50, installed_at: 1 })], [1, row({ rom_id: 1, last_played_at: 10, installed_at: 9 })]]);
    const rowFor = (g: { rom_id: number }) => rows.get(g.rom_id) ?? null;
    expect(driveGrid(games, '', 'played', rowFor).map((g) => g.name)).toEqual(['Charlie', 'Bravo', 'Alpha']);
    expect(driveGrid(games, '', 'installed', rowFor).map((g) => g.name)).toEqual(['Bravo', 'Charlie', 'Alpha']);
    expect(driveGrid(games, '', 'title', rowFor).map((g) => g.name)).toEqual(['Alpha', 'Bravo', 'Charlie']);
    expect(driveGrid(games, '', 'platform', rowFor).map((g) => g.name)).toEqual(['Charlie', 'Bravo', 'Alpha']);
    expect(driveGrid(games, 'ar', 'title', rowFor).map((g) => g.name)).toEqual(['Charlie']);
  });

  it('says why a drive grid is empty', () => {
    expect(driveEmptyState('Couch kit', false, '')).toEqual({ title: 'Couch kit is not connected', body: 'Plug it in to see its games.' });
    expect(driveEmptyState('Couch kit', true, 'q')).toEqual({ title: 'No games on this drive match “q”', body: 'Press (Y) to change the search.' });
    expect(driveEmptyState('Couch kit', true, '')).toEqual({ title: 'Nothing on this drive yet', body: 'Backup games from Server or Library.' });
  });
});
