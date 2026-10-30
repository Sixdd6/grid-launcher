import { describe, expect, it } from 'vitest';
import {
  cardDisplayLabel,
  CARD_DISPLAYS,
  clampShape,
  normalizeCardDisplay,
  columnsLayout,
  DEFAULT_SHAPE,
  GAP_PX,
  MIN_COVER_PX,
  neighbor,
  rowsLayout,
  visibleRange,
  type Rect,
} from './layout';

const LABEL = 32;
const rows = (shapes: (number | undefined)[], width = 1000, target = 213) =>
  rowsLayout({ shapes, width, labelRow: LABEL, target });
const cols = (shapes: (number | undefined)[], width = 1000, minColumn = 160) =>
  columnsLayout({ shapes, width, labelRow: LABEL, minColumn });

/** The cards of a layout grouped by their top edge. */
function byRow(rects: Rect[]): Rect[][] {
  const map = new Map<number, Rect[]>();
  for (const r of rects) map.set(r.y, [...(map.get(r.y) ?? []), r]);
  return [...map.values()];
}

describe('clampShape', () => {
  it('treats an unknown or broken shape as portrait, and clamps extremes', () => {
    expect(clampShape(undefined)).toBe(DEFAULT_SHAPE);
    expect(clampShape(Number.NaN)).toBe(DEFAULT_SHAPE);
    expect(clampShape(0)).toBe(DEFAULT_SHAPE);
    expect(clampShape(0.1)).toBe(0.5);
    expect(clampShape(9)).toBe(2);
  });
});

describe('Equal height (rows)', () => {
  it('sizes every card to its art: one height per row, width from the shape', () => {
    const shapes = [0.73, 1.86, 1, 0.73, 1.33, 0.73, 0.73, 1, 0.73, 1.43];
    const { rects } = rows(shapes);
    for (const [i, r] of rects.entries()) {
      // Within a pixel of the art's shape.
      expect(Math.abs(r.w / r.h - shapes[i])).toBeLessThan(0.05);
    }
    for (const row of byRow(rects)) expect(new Set(row.map((r) => r.h)).size).toBe(1);
  });

  it('keeps every card in a long row within a pixel of its art, the last one too', () => {
    // A cover drawn uncropped in a box wider than its art leaves an edge, and
    // carrying the rounding along a 14-card row used to pile pixels onto the
    // last card.
    const shapes = Array.from({ length: 60 }, (_, i) => [0.75, 0.7, 0.75, 0.77][i % 4]);
    for (let width = 1200; width <= 1800; width += 7) {
      const { rects } = rows(shapes, width, 120);
      for (const [i, r] of rects.entries()) {
        // A full row's height is fractional before rounding, so allow for it.
        expect(Math.abs(r.w - r.h * shapes[i])).toBeLessThanOrEqual(1.5);
      }
    }
  });

  it('stretches each full row to the width, and leaves the last row at the target', () => {
    const { rects } = rows(Array.from({ length: 20 }, () => 0.73));
    const all = byRow(rects);
    for (const row of all.slice(0, -1)) {
      const last = row[row.length - 1];
      expect(last.x + last.w).toBe(1000);
    }
    const lastRow = all[all.length - 1];
    expect(lastRow[0].h).toBe(213);
  });

  it('keeps rows between 75% and 130% of the target, and every cover at least the minimum', () => {
    const shapes = [0.73, 2, 0.5, 2, 2, 0.73, 1, 0.5, 0.5, 2, 1.86, 0.73];
    for (const width of [300, 640, 1000, 1600]) {
      for (const target of [160, 213, 267]) {
        const { rects } = rows(shapes, width, target);
        for (const r of rects) {
          expect(r.h).toBeGreaterThanOrEqual(Math.min(MIN_COVER_PX, Math.floor(width / 2)));
          expect(r.h).toBeLessThanOrEqual(Math.round(target * 1.3));
          expect(r.x + r.w).toBeLessThanOrEqual(width);
        }
      }
    }
  });

  it('places rows one under another with the label row and gap between them', () => {
    const { rects, height } = rows(Array.from({ length: 12 }, () => 1));
    const [first, second] = byRow(rects);
    expect(second[0].y).toBe(first[0].y + first[0].h + LABEL + 20);
    const last = rects[rects.length - 1];
    expect(height).toBe(last.y + last.h + LABEL);
  });

  it('lays unknown shapes out as portrait until they load', () => {
    const { rects } = rows([undefined, undefined]);
    expect(rects[0].w / rects[0].h).toBeCloseTo(DEFAULT_SHAPE, 1);
  });
});

describe('Equal width (columns)', () => {
  it('gives every card the column width and a height from its art', () => {
    const shapes = [0.73, 1, 0.62, 0.73, 1, 0.73];
    const { rects } = cols(shapes);
    const widths = new Set(rects.map((r) => r.w));
    expect(widths.size).toBe(1);
    for (const [i, r] of rects.entries()) expect(Math.abs(r.w / r.h - shapes[i])).toBeLessThan(0.02);
  });

  it('puts each card in the shortest column', () => {
    // Five columns of 187px at 1000px; a tall card then four short ones.
    const { rects } = cols([0.5, 1, 1, 1, 1, 1]);
    expect(rects[5].x).toBe(rects[1].x);
    expect(rects[5].y).toBeGreaterThan(0);
  });

  it('spans two columns for art too wide to be the minimum height in one', () => {
    const { rects } = cols([0.73, 1.86, 0.73], 1000, 160);
    const column = rects[0].w;
    expect(rects[1].w).toBe(Math.round(2 * column + GAP_PX));
    expect(rects[1].h).toBeGreaterThanOrEqual(MIN_COVER_PX);
  });

  it('never lays a cover out shorter than the minimum', () => {
    const shapes = [2, 2, 1.86, 1.5, 1.2, 1, 2];
    for (const width of [400, 800, 1600]) {
      for (const minColumn of [120, 160, 200]) {
        for (const r of cols(shapes, width, minColumn).rects) expect(r.h).toBeGreaterThanOrEqual(MIN_COVER_PX);
      }
    }
  });
});

describe('visibleRange', () => {
  it('is the cards meeting the band, label included', () => {
    const layout = rows(Array.from({ length: 50 }, () => 0.75));
    const range = visibleRange(layout, LABEL, 0, 300);
    expect(range.start).toBe(0);
    expect(range.end).toBeGreaterThan(0);
    expect(range.end).toBeLessThan(50);
    expect(visibleRange(layout, LABEL, 1e7, 2e7)).toEqual({ start: 0, end: 0 });
  });
});

describe('neighbor', () => {
  const grid: Rect[] = [
    { x: 0, y: 0, w: 100, h: 100 },
    { x: 110, y: 0, w: 200, h: 100 },
    { x: 320, y: 0, w: 100, h: 100 },
    { x: 0, y: 150, w: 150, h: 100 },
    { x: 160, y: 150, w: 100, h: 100 },
    { x: 270, y: 150, w: 150, h: 100 },
  ];

  it('steps left and right in reading order, stopping at the ends', () => {
    expect(neighbor(grid, 0, 'left')).toBe(0);
    expect(neighbor(grid, 2, 'right')).toBe(3);
    expect(neighbor(grid, 5, 'right')).toBe(5);
  });

  it('goes to the overlapping card below or above, preferring the most overlap', () => {
    expect(neighbor(grid, 1, 'down')).toBe(4);
    expect(neighbor(grid, 2, 'down')).toBe(5);
    expect(neighbor(grid, 3, 'up')).toBe(0);
    expect(neighbor(grid, 0, 'up')).toBe(0);
    expect(neighbor(grid, 5, 'down')).toBe(5);
  });

  it('moves within a column in Equal width', () => {
    const { rects } = cols([0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73, 0.73]);
    const below = neighbor(rects, 0, 'down');
    expect(rects[below].x).toBe(rects[0].x);
    expect(rects[below].y).toBeGreaterThan(rects[0].y);
  });
});

describe('Game card display setting', () => {
  it('reads columns, and anything else as rows (the Rust normaliser)', () => {
    expect(normalizeCardDisplay('columns')).toBe('columns');
    expect(normalizeCardDisplay(' columns ')).toBe('columns');
    expect(normalizeCardDisplay('Columns')).toBe('rows');
    expect(normalizeCardDisplay('')).toBe('rows');
  });

  it('names both layouts for the select', () => {
    expect(CARD_DISPLAYS.map(cardDisplayLabel)).toEqual(['Equal height', 'Equal width']);
  });
});
