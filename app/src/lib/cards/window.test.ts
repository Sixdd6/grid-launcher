import { describe, expect, it } from 'vitest';
import { CARD_GAP_PX, TITLE_ROW_HEIGHT_PX } from './size';
import {
  BUFFER_ROWS,
  GRID_GAP_PX,
  INITIAL_WINDOW_CARDS,
  initialRange,
  rowMetrics,
  scrollTopForIndex,
  windowRange,
} from './window';

describe('rowMetrics', () => {
  it('is a 3:4 cover plus the gap and the title strip, then the grid gap', () => {
    const { rowHeight, pitch } = rowMetrics(150);
    expect(rowHeight).toBe(200 + CARD_GAP_PX + TITLE_ROW_HEIGHT_PX);
    expect(pitch).toBe(rowHeight + GRID_GAP_PX);
  });

  it('rounds to whole px, so the layout and the math agree over many rows', () => {
    // 188.25 / (3/4) = 251 exactly; 188.3 gives 251.07 and rounds to 251.
    expect(rowMetrics(188.3).rowHeight).toBe(251 + CARD_GAP_PX + TITLE_ROW_HEIGHT_PX);
    expect(Number.isInteger(rowMetrics(181.7).rowHeight)).toBe(true);
  });

  it('takes the gap as a parameter', () => {
    expect(rowMetrics(150, 0).pitch).toBe(rowMetrics(150, 0).rowHeight);
  });
});

describe('initialRange', () => {
  it('renders a short list whole, with no padding', () => {
    expect(initialRange(3)).toEqual({ start: 0, end: 3, padTop: 0, padBottom: 0 });
  });

  it('renders the first INITIAL_WINDOW_CARDS of a long list', () => {
    expect(initialRange(2500)).toEqual({ start: 0, end: INITIAL_WINDOW_CARDS, padTop: 0, padBottom: 0 });
  });

  it('fills whole rows at the usual column counts', () => {
    for (const columns of [1, 2, 3, 4, 6, 8, 12]) expect(INITIAL_WINDOW_CARDS % columns).toBe(0);
  });
});

describe('windowRange', () => {
  const pitch = 100;
  const base = { pitch, columns: 5, viewportHeight: 300, bufferRows: 2 };

  it('renders every card of a platform that fits on screen, with no padding', () => {
    expect(windowRange({ ...base, offset: 0, count: 12 })).toEqual({
      start: 0,
      end: 12,
      padTop: 0,
      padBottom: 0,
    });
  });

  it('renders the visible rows plus the buffer at the top of a long list', () => {
    // Rows 0-2 are visible, rows 3-4 are the buffer below; 200 rows in all.
    expect(windowRange({ ...base, offset: 0, count: 1000 })).toEqual({
      start: 0,
      end: 25,
      padTop: 0,
      padBottom: 195 * pitch,
    });
  });

  it('carries the rows above and below the window in the padding', () => {
    // Visible: rows 10-13 (offset 1050 to 1350). Buffer: 8-9 and 14-15.
    const range = windowRange({ ...base, offset: 1050, count: 1000 });
    expect(range).toEqual({ start: 40, end: 80, padTop: 8 * pitch, padBottom: 184 * pitch });
    // The padding and the rendered rows add up to the whole list.
    const renderedRows = (range.end - range.start) / base.columns;
    expect(range.padTop + renderedRows * pitch + range.padBottom).toBe(200 * pitch);
  });

  it('starts at the top while the grid is still below the visible top', () => {
    // A header 250px tall pushes the grid down: only row 0 overlaps.
    expect(windowRange({ ...base, offset: -250, count: 1000 })).toMatchObject({
      start: 0,
      end: 15,
      padTop: 0,
    });
  });

  it('ends with the last, partial row and no bottom padding', () => {
    const range = windowRange({ ...base, offset: 19_800, count: 998 });
    expect(range.end).toBe(998);
    expect(range.padBottom).toBe(0);
    expect(range.start % base.columns).toBe(0);
  });

  it('still renders the end of the list for an offset past it', () => {
    const range = windowRange({ ...base, offset: 1_000_000, count: 12 });
    expect(range.end).toBe(12);
    expect(range.start).toBeLessThan(12);
  });

  it('renders nothing for an empty list', () => {
    expect(windowRange({ ...base, offset: 0, count: 0 })).toEqual({
      start: 0,
      end: 0,
      padTop: 0,
      padBottom: 0,
    });
  });

  it('falls back to the initial range without a measurement', () => {
    expect(windowRange({ ...base, pitch: 0, offset: 0, count: 2500 })).toEqual(initialRange(2500));
    expect(windowRange({ ...base, viewportHeight: 0, offset: 0, count: 2500 })).toEqual(
      initialRange(2500)
    );
  });

  it('buffers BUFFER_ROWS by default', () => {
    const range = windowRange({ pitch, viewportHeight: 300, columns: 1, offset: 1000, count: 1000 });
    expect(range.start).toBe(10 - BUFFER_ROWS);
    expect(range.end).toBe(13 + BUFFER_ROWS);
  });
});

describe('scrollTopForIndex', () => {
  const metrics = { rowHeight: 90, pitch: 100 };
  const base = { columns: 4, metrics, rowsTop: 50, viewportHeight: 300 };

  it('leaves a fully visible row where it is', () => {
    expect(scrollTopForIndex({ ...base, index: 9, scrollTop: 200 })).toBe(200);
  });

  it('aligns a row above the visible part with its top', () => {
    // Index 9 is row 2, which starts at 50 + 200.
    expect(scrollTopForIndex({ ...base, index: 9, scrollTop: 1000 })).toBe(250);
  });

  it('aligns a row below the visible part with its bottom', () => {
    // Index 40 is row 10: top 1050, bottom 1140.
    expect(scrollTopForIndex({ ...base, index: 40, scrollTop: 0 })).toBe(1140 - 300);
  });

  it('shows a row taller than the visible part from its top', () => {
    expect(scrollTopForIndex({ ...base, viewportHeight: 50, index: 40, scrollTop: 0 })).toBe(1050);
  });
});
