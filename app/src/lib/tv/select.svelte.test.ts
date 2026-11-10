import { beforeEach, describe, expect, it, vi } from 'vitest';

const toasts = vi.hoisted(() => ({ calls: [] as unknown[][] }));
vi.mock('../stores/toasts.svelte', () => ({
  pushToast: (...args: unknown[]) => {
    toasts.calls.push(args);
    return 1;
  },
}));

import {
  currentSelHints,
  registerSelectGrid,
  select,
  selAct,
  selActOutcome,
  selAll,
  selBar,
  selEnd,
  selHints,
  selKeep,
  selPick,
  selToggle,
  type SelectGrid,
} from './select.svelte';

function grid(over: Partial<SelectGrid> = {}): SelectGrid & { acted: string[][] } {
  const acted: string[][] = [];
  return {
    games: () => [
      { id: 'a', size: 1_073_741_824 },
      { id: 'b', size: 2_147_483_648 },
      { id: 'c', size: 536_870_912 },
    ],
    all: () => [
      { id: 'a', size: 1_073_741_824 },
      { id: 'b', size: 2_147_483_648 },
      { id: 'c', size: 536_870_912 },
      { id: 'd', size: 1_073_741_824 },
    ],
    drive: () => false,
    connected: () => true,
    act: (ids) => acted.push(ids),
    acted,
    ...over,
  };
}

describe('select mode', () => {
  beforeEach(() => {
    selEnd();
    toasts.calls = [];
  });

  it('starts with nothing picked and RT turns it off again (item 439)', () => {
    selToggle();
    expect(select.on).toBe(true);
    expect(select.ids).toEqual([]);
    selPick('a');
    selToggle();
    expect(select.on).toBe(false);
    selToggle();
    expect(select.ids).toEqual([]);
  });

  it('A picks and unpicks a cover (item 440)', () => {
    selToggle();
    selPick('a');
    selPick('b');
    expect(select.picked('a')).toBe(true);
    selPick('a');
    expect(select.ids).toEqual(['b']);
  });

  it('does nothing while it is off', () => {
    selPick('a');
    selAll(['a', 'b']);
    expect(select.ids).toEqual([]);
  });

  it('X selects every game the grid shows, then clears them (item 442)', () => {
    selToggle();
    selPick('b');
    selAll(['a', 'b', 'c']);
    expect([...select.ids].sort()).toEqual(['a', 'b', 'c']);
    selAll(['a', 'b', 'c']);
    expect(select.ids).toEqual([]);
  });

  it('drops picks the rail row no longer lists (a game left the drive)', () => {
    selToggle();
    selAll(['a', 'b', 'c']);
    selKeep(['a', 'c']);
    expect(select.ids).toEqual(['a', 'c']);
  });

  it('keeps picks a search hides; the summary counts them; Select all N counts the shown covers (consultant, after P13)', () => {
    const g = grid({ games: () => [{ id: 'a', size: 1_073_741_824 }] });
    const drop = registerSelectGrid(g);
    selToggle();
    selPick('a');
    selPick('d');
    // The search shows only "a"; the grids keep against the whole list.
    selKeep(g.all().map((x) => x.id));
    expect(select.ids).toEqual(['a', 'd']);
    const bar = selBar(select.ids, g.games(), g.all(), false);
    expect(bar.summary).toBe('2 selected · 2 GB');
    expect(bar.x).toBe('All 1 selected · Clear');
    // X's hint reads against the shown covers too.
    expect(currentSelHints(null, 'Select')?.[1]).toEqual({ button: 'X', label: 'Clear all' });
    // Y acts on every pick, the hidden one too.
    selAct(g);
    expect(g.acted).toEqual([['a', 'd']]);
    drop();
  });

  it('decides what Y does (items 443, 447–449)', () => {
    expect(selActOutcome(0, false, true)).toBe('pick-first');
    expect(selActOutcome(0, true, false)).toBe('pick-first');
    expect(selActOutcome(2, false, false)).toBe('offline');
    // A drive installs or removes without a server.
    expect(selActOutcome(2, true, false)).toBe('drive');
    expect(selActOutcome(2, false, true)).toBe('backup');
  });

  it('Y with nothing picked says how to pick (item 447)', () => {
    const g = grid();
    selToggle();
    selAct(g);
    expect(toasts.calls).toEqual([['Pick games first', 'success', undefined, { sub: 'A picks the focused game; X picks them all.' }]]);
    expect(g.acted).toEqual([]);
  });

  it('Y offline says backing up needs a server (item 448)', () => {
    const g = grid({ connected: () => false });
    selToggle();
    selPick('a');
    selAct(g);
    expect(toasts.calls).toEqual([['Backing up needs a server.', 'error']]);
    expect(g.acted).toEqual([]);
  });

  it('Y hands the picked games to the grid, in the order they were picked', () => {
    const g = grid();
    selToggle();
    selPick('c');
    selPick('a');
    selAct(g);
    expect(g.acted).toEqual([['c', 'a']]);
  });

  it('acting or a drive going away ends it (item 452)', () => {
    selToggle();
    selPick('a');
    selEnd();
    expect(select.on).toBe(false);
    expect(select.ids).toEqual([]);
  });

  it('words the bar under the grid (item 445)', () => {
    const games = grid().games();
    const all = grid().all();
    expect(selBar(['a'], games, all, false)).toEqual({ summary: '1 selected · 1 GB', x: 'Select all 3', y: 'Backup to drive…', rt: 'Done' });
    expect(selBar(['a', 'b', 'c'], games, all, true).x).toBe('All 3 selected · Clear');
    expect(selBar(['a', 'b', 'c'], games, all, true).y).toBe('Install or remove');
  });

  it('gives select mode its own hints (item 446)', () => {
    expect(selHints({ cover: true, onPicked: false, a: 'Details', all: false, drive: false })).toEqual([
      { button: 'A', label: 'Pick' },
      { button: 'X', label: 'Select all' },
      { button: 'Y', label: 'Backup to drive…' },
      { button: 'RT', label: 'Done' },
    ]);
    const h = selHints({ cover: true, onPicked: true, a: 'Details', all: true, drive: true });
    expect(h.map((x) => x.label)).toEqual(['Unpick', 'Clear all', 'Install or remove', 'Done']);
  });

  it('reads the hints from the registered grid and the focused cover', () => {
    const drop = registerSelectGrid(grid());
    expect(currentSelHints(null, 'Select')).toBeNull();
    selToggle();
    const cover = { getAttribute: (n: string) => (n === 'data-sel' ? 'a' : null) } as unknown as HTMLElement;
    selPick('a');
    expect(currentSelHints(cover, 'Details')?.[0]).toEqual({ button: 'A', label: 'Unpick' });
    expect(currentSelHints(null, 'Select')?.[0]).toEqual({ button: 'A', label: 'Select' });
    drop();
    expect(currentSelHints(cover, 'Details')).toBeNull();
  });
});
