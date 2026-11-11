import { describe, expect, it } from 'vitest';
import { columnsLayout, rowsLayout, type Rect } from '../../cards/layout';
import { layoutMove, windowIndexes } from './grid';

// Two justified rows: three covers, then two (a landscape one first).
const rows: Rect[] = [
  { x: 0, y: 0, w: 100, h: 130 },
  { x: 110, y: 0, w: 100, h: 130 },
  { x: 220, y: 0, w: 100, h: 130 },
  { x: 0, y: 160, w: 200, h: 120 },
  { x: 210, y: 160, w: 90, h: 120 },
];

describe('layoutMove (Equal height)', () => {
  it('steps along a row and leaves at its ends', () => {
    expect(layoutMove(rows, 1, 'left')).toBe(0);
    expect(layoutMove(rows, 1, 'right')).toBe(2);
    expect(layoutMove(rows, 0, 'left')).toBe('edge');
    expect(layoutMove(rows, 2, 'right')).toBe('edge');
    expect(layoutMove(rows, 3, 'left')).toBe('edge');
  });

  it('goes up and down to the cover sharing the most width', () => {
    // Cover 1 (110–210) shares 90px with 3 (0–200) and 0px with 4 (210–300).
    expect(layoutMove(rows, 1, 'down')).toBe(3);
    expect(layoutMove(rows, 2, 'down')).toBe(4);
    // Cover 3 (0–200) shares 100px with 0 and 90px with 1.
    expect(layoutMove(rows, 3, 'up')).toBe(0);
    expect(layoutMove(rows, 4, 'up')).toBe(2);
  });

  it('leaves from the top and bottom rows', () => {
    expect(layoutMove(rows, 0, 'up')).toBe('edge');
    expect(layoutMove(rows, 3, 'down')).toBe('edge');
  });

  it('goes down into a shorter last row by distance when nothing is below', () => {
    const short: Rect[] = [...rows.slice(0, 3), { x: 0, y: 160, w: 100, h: 130 }];
    expect(layoutMove(short, 2, 'down')).toBe(3);
  });

  it('leaves an empty grid or a missing index', () => {
    expect(layoutMove([], 0, 'down')).toBe('edge');
    expect(layoutMove(rows, 9, 'down')).toBe('edge');
  });
});

describe('layoutMove (Equal width)', () => {
  // Masonry: column 0 holds a tall and a short cover, column 1 the reverse.
  const cols: Rect[] = [
    { x: 0, y: 0, w: 100, h: 200 },
    { x: 110, y: 0, w: 100, h: 80 },
    { x: 110, y: 100, w: 100, h: 150 },
    { x: 0, y: 220, w: 100, h: 60 },
  ];

  it('moves up and down inside a column', () => {
    expect(layoutMove(cols, 1, 'down')).toBe(2);
    expect(layoutMove(cols, 0, 'down')).toBe(3);
    expect(layoutMove(cols, 3, 'up')).toBe(0);
  });

  it('moves sideways to the cover sharing the most height', () => {
    // Cover 0 (0–200) shares 80px with 1 and 100px with 2, both one column
    // over: the one sharing more wins.
    expect(layoutMove(cols, 0, 'right')).toBe(2);
    expect(layoutMove(cols, 2, 'left')).toBe(0);
    expect(layoutMove(cols, 3, 'right')).toBe(2);
    expect(layoutMove(cols, 1, 'right')).toBe('edge');
  });

  it('works on a real layout', () => {
    const shapes = [0.75, 1.4, 0.7, 0.75, 0.75, 0.75];
    const r = rowsLayout({ shapes, width: 600, labelRow: 20, target: 200 }).rects;
    for (let i = 0; i < r.length; i++) {
      const right = layoutMove(r, i, 'right');
      if (right !== 'edge') expect(r[right].y).toBe(r[i].y);
    }
    const c = columnsLayout({ shapes, width: 600, labelRow: 20, minColumn: 150 }).rects;
    expect(layoutMove(c, 0, 'right')).not.toBe('edge');
  });
});

describe('windowIndexes', () => {
  it('is the range when focus is in it or there is none', () => {
    expect(windowIndexes({ start: 2, end: 5 }, null, 10)).toEqual([2, 3, 4]);
    expect(windowIndexes({ start: 2, end: 5 }, 3, 10)).toEqual([2, 3, 4]);
  });

  it('adds a focused cover outside the range, in order', () => {
    expect(windowIndexes({ start: 2, end: 5 }, 0, 10)).toEqual([0, 2, 3, 4]);
    expect(windowIndexes({ start: 2, end: 5 }, 8, 10)).toEqual([2, 3, 4, 8]);
  });

  it('clamps to the list', () => {
    expect(windowIndexes({ start: 0, end: 48 }, null, 3)).toEqual([0, 1, 2]);
    expect(windowIndexes({ start: 0, end: 2 }, 7, 3)).toEqual([0, 1]);
  });
});
