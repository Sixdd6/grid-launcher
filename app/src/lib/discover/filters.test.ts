import { describe, expect, it } from 'vitest';
import type { GameSummary, RecentPlatform } from '../api';
import { filterPills, keptFilter, listKey, nextFilter, shownList } from './filters';

const snes: RecentPlatform = { id: 1, label: 'SNES' };
const arcade: RecentPlatform = { id: 2, label: 'Arcade' };
const game = (id: number) => ({ id }) as GameSummary;

describe('filterPills', () => {
  it('shows every platform when there are two or more, in their order', () => {
    expect(filterPills([arcade, snes])).toEqual([arcade, snes]);
  });

  it('hides the row for one platform or none', () => {
    expect(filterPills([snes])).toEqual([]);
    expect(filterPills([])).toEqual([]);
  });
});

describe('nextFilter', () => {
  it('selects a platform, one at a time', () => {
    expect(nextFilter(null, 1)).toBe(1);
    expect(nextFilter(1, 2)).toBe(2);
  });

  it('clears on the pressed pill again or on All', () => {
    expect(nextFilter(1, 1)).toBeNull();
    expect(nextFilter(1, null)).toBeNull();
    expect(nextFilter(null, null)).toBeNull();
  });
});

describe('keptFilter', () => {
  it('keeps a platform still offered', () => {
    expect(keptFilter(2, [snes, arcade])).toBe(2);
  });

  it('drops one no longer offered, or when the row hides', () => {
    expect(keptFilter(3, [snes, arcade])).toBeNull();
    expect(keptFilter(1, [snes])).toBeNull();
    expect(keptFilter(null, [snes, arcade])).toBeNull();
  });
});

describe('shownList and listKey', () => {
  it('show the feed list until a filtered one arrives', () => {
    const all = [game(1), game(2)];
    expect(shownList(all, null)).toBe(all);
    expect(listKey(null)).toBe('all');
    const filtered = { id: 2, games: [game(2)] };
    expect(shownList(all, filtered)).toEqual([game(2)]);
    expect(listKey(filtered)).toBe('platform-2');
  });
});
