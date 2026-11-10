import { describe, expect, it } from 'vitest';
import { gridColumns, gridMove, gridWindow, INITIAL_WINDOW } from './grid';

describe('gridColumns', () => {
  it('fits 10rem columns with at least a 1.2rem gap', () => {
    // 1000px wide at 20px rem: (1000 + 24) / (200 + 24) = 4.57 → 4.
    expect(gridColumns(1000, 200, 24)).toBe(4);
    expect(gridColumns(150, 200, 24)).toBe(1);
    expect(gridColumns(0, 200, 24)).toBe(1);
  });
});

describe('gridMove', () => {
  // 4 columns, 10 covers: rows 0–3, 4–7, 8–9.
  it('moves inside the grid', () => {
    expect(gridMove(5, 'left', 4, 10)).toBe(4);
    expect(gridMove(5, 'right', 4, 10)).toBe(6);
    expect(gridMove(5, 'up', 4, 10)).toBe(1);
    expect(gridMove(1, 'down', 4, 10)).toBe(5);
  });

  it('answers edge off every side (items 88–90)', () => {
    expect(gridMove(4, 'left', 4, 10)).toBe('edge');
    expect(gridMove(3, 'right', 4, 10)).toBe('edge');
    expect(gridMove(9, 'right', 4, 10)).toBe('edge');
    expect(gridMove(2, 'up', 4, 10)).toBe('edge');
    expect(gridMove(8, 'down', 4, 10)).toBe('edge');
  });

  it('down into a shorter last row lands on its last cover', () => {
    expect(gridMove(7, 'down', 4, 10)).toBe(9);
  });

  it('an empty grid is all edge', () => {
    expect(gridMove(0, 'down', 4, 0)).toBe('edge');
  });
});

describe('gridWindow (risk R8)', () => {
  // 2500 covers, 6 columns (417 rows), 300px a row, a 900px band.
  const base = { count: 2500, columns: 6, pitch: 300, top: 0, bottom: 900, focus: null };

  it('renders the first covers before measuring', () => {
    expect(gridWindow({ ...base, pitch: 0 })).toEqual({ start: 0, end: INITIAL_WINDOW, rowsBefore: 0, rowsAfter: 417 - 8 });
    expect(gridWindow({ ...base, count: 0 })).toEqual({ start: 0, end: 0, rowsBefore: 0, rowsAfter: 0 });
    expect(gridWindow({ ...base, count: 10, pitch: 0 })).toEqual({ start: 0, end: 10, rowsBefore: 0, rowsAfter: 0 });
  });

  it('renders the visible rows and two either side', () => {
    expect(gridWindow(base)).toEqual({ start: 0, end: 6 * 6, rowsBefore: 0, rowsAfter: 416 - 5 });
    // Scrolled to row 100: rows 98–105.
    expect(gridWindow({ ...base, top: 30000, bottom: 30900 })).toEqual({ start: 98 * 6, end: 106 * 6, rowsBefore: 98, rowsAfter: 416 - 105 });
  });

  it('ends at the last cover', () => {
    const w = gridWindow({ ...base, top: 124200, bottom: 125100 });
    expect(w.end).toBe(2500);
    expect(w.rowsAfter).toBe(0);
  });

  it('keeps the focused row and one row past it rendered', () => {
    // Focus on row 6, just past the buffer: the window grows to row 7.
    const w = gridWindow({ ...base, focus: 6 * 6 + 2 });
    expect(w.start).toBe(0);
    expect(w.end).toBe(8 * 6);
  });

  it('follows focus that is far from the band', () => {
    const w = gridWindow({ ...base, focus: 300 * 6 });
    expect(w.rowsBefore).toBe(298);
    expect(w.start).toBeLessThanOrEqual(300 * 6);
    expect(w.end).toBeGreaterThan(301 * 6);
  });
});
