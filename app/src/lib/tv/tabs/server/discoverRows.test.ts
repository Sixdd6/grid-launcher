import { describe, expect, it } from 'vitest';
import { gameFromRow, gameFromSummary } from '../../games';
import { row, summary } from '../../fixtures.test-util';
import { ago, daysSince, discoverShelves, EMPTY_SHELF, gamesWord, heroEmulator, heroLine, heroSummary } from './discoverRows';

const inst = (id: number, o = {}) => gameFromRow(row({ rom_id: id, title: `G${id}`, ...o }), new Set([3]));
const srvGame = (id: number) => gameFromSummary(summary({ id, name: `S${id}` }), 'SNES', 'snes', null, new Set());

describe('discoverShelves (items 164–169)', () => {
  it('rows in order with their extra text; Updates only when any', () => {
    const lib = [inst(1, { last_played_at: 100 }), inst(2), inst(3, { last_played_at: 300 })];
    const shelves = discoverShelves({ library: lib, newReleases: [srvGame(10)], recentlyAdded: [srvGame(11)] });
    expect(shelves.map((s) => [s.id, s.name, s.extra])).toEqual([
      ['c', 'Continue playing', null],
      ['n', 'New releases', 'the last six months, on your systems'],
      ['r', 'Recently added', 'to your server'],
      ['u', 'Updates', '1 game'],
      ['l', 'Your library', '3 games'],
    ]);
    expect(discoverShelves({ library: [inst(1)], newReleases: [], recentlyAdded: [] }).map((s) => s.id)).toEqual(['c', 'n', 'r', 'l']);
  });

  it('Continue playing: installed games with play history, most recent first', () => {
    const lib = [inst(1, { last_played_at: 100 }), inst(2), inst(4, { last_played_at: 300 })];
    expect(discoverShelves({ library: lib, newReleases: [], recentlyAdded: [] })[0].games.map((g) => g.romId)).toEqual([4, 1]);
  });

  it('New releases as the feed orders them; Recently added leaves out what New releases shows', () => {
    const s = discoverShelves({ library: [], newReleases: [srvGame(10), srvGame(12)], recentlyAdded: [srvGame(12), srvGame(11)] });
    expect(s[1].games.map((g) => g.romId)).toEqual([10, 12]);
    expect(s[2].games.map((g) => g.romId)).toEqual([11]);
  });

  it('Updates: installed games with an update; Your library: every installed game', () => {
    const s = discoverShelves({ library: [inst(2), inst(3)], newReleases: [], recentlyAdded: [] });
    expect(s.find((x) => x.id === 'u')?.games.map((g) => g.romId)).toEqual([3]);
    expect(s.find((x) => x.id === 'l')?.games.map((g) => g.romId)).toEqual([2, 3]);
  });

  it('the empty row text and counts', () => {
    expect(EMPTY_SHELF).toBe('Nothing here yet.');
    expect(gamesWord(1)).toBe('1 game');
    expect(gamesWord(0)).toBe('0 games');
  });
});

describe('the hero (items 161–162)', () => {
  it('ago: today, yesterday, days under 14, else weeks', () => {
    expect([0, 1, 2, 13, 14, 20, 21].map(ago)).toEqual(['today', 'yesterday', '2 days ago', '13 days ago', '2 weeks ago', '3 weeks ago', '3 weeks ago']);
    expect(daysSince(1000, 1000 + 86_400 * 3 + 5)).toBe(3);
  });

  it('the line per state', () => {
    const now = 1_000_000;
    expect(heroLine(inst(1, { missing: true }), null, 'Snes9x', now)).toBe('Files not found');
    expect(heroLine(inst(1, { last_played_at: now - 86_400 }), null, 'Snes9x', now)).toBe('Played yesterday · Snes9x');
    expect(heroLine(inst(1), null, 'RetroArch · snes9x', now)).toBe('Installed · RetroArch · snes9x');
    expect(heroLine(inst(1), null, null, now)).toBe('Installed · No emulator');
    expect(heroLine(srvGame(5), { queued: false, pct: 62 }, null, now)).toBe('Installing… 62%');
    expect(heroLine(srvGame(5), { queued: true, pct: 0 }, null, now)).toBe('Queued');
    const released = gameFromSummary(summary({ first_release_date: Date.UTC(2026, 10, 7) / 1000 }), 'SNES', 'snes', null, new Set());
    expect(heroLine(released, null, null, now)).toBe('Released 7 Nov 2026 · Not installed');
    const undated = gameFromSummary(summary({ first_release_date: null }), 'SNES', 'snes', null, new Set());
    expect(heroLine(undated, null, null, now)).toBe('Not installed');
  });

  it('the emulator named: the launch line, "No emulator", or a native game\'s tool', () => {
    expect(heroEmulator('Snes9x', false, '')).toBe('Snes9x');
    expect(heroEmulator('No default emulator', false, '')).toBeNull();
    expect(heroEmulator('', true, 'GE-Proton9-20')).toBe('GE-Proton9-20');
    expect(heroEmulator('', false, '')).toBe('Native');
  });

  it('the summary is its first paragraph', () => {
    expect(heroSummary('\nOne.\nTwo.')).toBe('One.');
    expect(heroSummary('')).toBe('');
  });
});
