import { describe, expect, it } from 'vitest';
import {
  allPicked,
  clear,
  EMPTY_SELECTION,
  isPicked,
  keepOnly,
  leavesSelectMode,
  pick,
  selectAll,
  selectAllLabel,
  selectionSummary,
  toggle,
  totalBytes,
} from './selection';

const VISIBLE = [10, 11, 12, 13, 14];

describe('toggle', () => {
  it('picks, then unpicks, and moves the anchor', () => {
    const one = toggle(EMPTY_SELECTION, 12);
    expect(one).toEqual({ ids: [12], anchor: 12 });
    expect(isPicked(one, 12)).toBe(true);
    const none = toggle(one, 12);
    expect(none.ids).toEqual([]);
    expect(none.anchor).toBe(12);
  });
});

describe('pick', () => {
  it('toggles on a plain click', () => {
    expect(pick(EMPTY_SELECTION, 11, false, VISIBLE).ids).toEqual([11]);
  });

  it('picks the range from the anchor in the visible order on Shift-click, either direction', () => {
    const start = pick(EMPTY_SELECTION, 11, false, VISIBLE);
    expect(pick(start, 13, true, VISIBLE).ids).toEqual([11, 12, 13]);
    const back = pick(pick(EMPTY_SELECTION, 14, false, VISIBLE), 12, true, VISIBLE);
    expect(back.ids).toEqual([14, 12, 13]);
  });

  it('never unpicks inside a Shift range and keeps earlier picks', () => {
    let sel = pick(EMPTY_SELECTION, 10, false, VISIBLE);
    sel = pick(sel, 12, false, VISIBLE);
    sel = pick(sel, 14, true, VISIBLE);
    expect([...sel.ids].sort()).toEqual([10, 12, 13, 14]);
    expect(sel.anchor).toBe(14);
  });

  it('falls back to a toggle with no anchor, or an anchor no longer visible', () => {
    expect(pick(EMPTY_SELECTION, 12, true, VISIBLE).ids).toEqual([12]);
    const gone = { ids: [99], anchor: 99 };
    expect(pick(gone, 12, true, VISIBLE).ids).toEqual([99, 12]);
  });
});

describe('select all and clear', () => {
  it('picks every visible cover, then unpicks them once all are picked', () => {
    const all = selectAll(toggle(EMPTY_SELECTION, 12), VISIBLE);
    expect([...all.ids].sort()).toEqual(VISIBLE);
    expect(allPicked(all, VISIBLE)).toBe(true);
    expect(selectAll(all, VISIBLE).ids).toEqual([]);
  });

  it('never calls an empty view all picked', () => {
    expect(allPicked(EMPTY_SELECTION, [])).toBe(false);
  });

  it('clear empties the selection', () => {
    expect(clear()).toEqual(EMPTY_SELECTION);
  });

  it('keepOnly drops games no longer listed', () => {
    const sel = { ids: [10, 99], anchor: 99 };
    expect(keepOnly(sel, VISIBLE)).toEqual({ ids: [10], anchor: null });
    const same = { ids: [10], anchor: 10 };
    expect(keepOnly(same, VISIBLE)).toBe(same);
  });
});

describe('totalBytes', () => {
  it('adds the picked sizes, treating unknown sizes as 0', () => {
    const sizes: Record<number, number> = { 10: 1000, 11: 0, 12: 500 };
    const sel = { ids: [10, 11, 12, 13], anchor: null };
    expect(totalBytes(sel, (id) => sizes[id] ?? 0)).toBe(1500);
  });
});

describe('Escape', () => {
  it('leaves select mode only when selecting and nothing else has the key', () => {
    expect(leavesSelectMode('Escape', true, false)).toBe(true);
    expect(leavesSelectMode('Escape', false, false)).toBe(false);
    expect(leavesSelectMode('Escape', true, true)).toBe(false);
    expect(leavesSelectMode('Enter', true, false)).toBe(false);
  });
});

describe('selection bar text', () => {
  it('shows the count and size, or the hint', () => {
    expect(selectionSummary(3, '4.2 GB')).toEqual({ title: '3 selected', note: '4.2 GB' });
    expect(selectionSummary(0, '0 B').title).toBe('Select games');
    expect(selectionSummary(0, '0 B').note).toContain('Shift-click');
  });

  it('labels Select all', () => {
    expect(selectAllLabel(11, false)).toBe('Select all 11');
    expect(selectAllLabel(11, true)).toBe('All 11 selected');
  });
});
