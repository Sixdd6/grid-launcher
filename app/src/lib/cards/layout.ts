// Where every card in a grid goes (look-and-feel plan 2026-10-30, "cards
// sized by their artwork"). Pure: the grid measures its width and knows each
// cover's shape; this module turns them into one rectangle per card.
//
// A card is the size of its artwork. Two layouts, chosen in Settings ›
// Appearance › Game card display:
// - `rows` ("Equal height"): justified rows. Every row is one height, each
//   card as wide as its art at that height, and the row stretched to the full
//   width (the last row is not).
// - `columns` ("Equal width"): masonry. Every column is one width, each card
//   as tall as its art, placed in the shortest column.
//
// The grid renders only the cards whose rectangles meet the visible part of
// its scroll container (`visibleRange`): a 2500-game platform costs what a
// screenful does, which WebKitGTK needs (2026-10-20).
import { CARD_CELL_ASPECT } from './size';

export const CARD_DISPLAYS = ['rows', 'columns'] as const;
export type CardDisplay = (typeof CARD_DISPLAYS)[number];

const DISPLAY_LABELS: Record<CardDisplay, string> = { rows: 'Equal height', columns: 'Equal width' };

/** The stored value, or `rows` for anything unrecognised. Matches the Rust
 *  `normalize_card_display`, including its case sensitivity. */
export function normalizeCardDisplay(raw: string): CardDisplay {
  return raw.trim() === 'columns' ? 'columns' : 'rows';
}

export function cardDisplayLabel(display: CardDisplay): string {
  return DISPLAY_LABELS[display];
}

/** One card's cover, in px from the grid's content box. The card itself is
 *  the cover plus its label row, below it. */
export type Rect = { x: number; y: number; w: number; h: number };

export type Layout = {
  rects: Rect[];
  /** From the first card's top to the last card's bottom, label included. */
  height: number;
};

/** A cover whose shape is not known yet (it has not loaded) is portrait. */
export const DEFAULT_SHAPE = CARD_CELL_ASPECT;
/** The narrowest and widest shapes a card takes, so one odd image (a banner,
 *  a sliver) cannot break a row or a column. */
export const MIN_SHAPE = 0.5;
export const MAX_SHAPE = 2;

/**
 * The shortest a cover is ever laid out, in px. It keeps the hover bar
 * (`ACTION_BAR_HEIGHT_PX`, the cover's bottom strip) clear of the card root's
 * centre, which the specs click to open Details (`size.test.ts`).
 */
export const MIN_COVER_PX = 112;

/** Equal height: how far a justified row may move from the target height. */
export const ROW_MIN_SCALE = 0.75;
export const ROW_MAX_SCALE = 1.3;

/** The gap between cards side by side, and between a card's label and the
 *  next card below, in px. */
export const GAP_PX = 16;
export const ROW_GAP_PX = 20;

/** Cards rendered before the grid has been measured: the first paint, and
 *  server-side rendering. */
export const INITIAL_WINDOW_CARDS = 96;

/** How far past the visible part cards are rendered, in px, so keyboard
 *  movement and an ordinary scroll find the next cards already there. */
export const BUFFER_PX = 600;

export function clampShape(shape: number | undefined): number {
  if (shape === undefined || !Number.isFinite(shape) || shape <= 0) return DEFAULT_SHAPE;
  return Math.min(MAX_SHAPE, Math.max(MIN_SHAPE, shape));
}

type Common = {
  shapes: readonly (number | undefined)[];
  /** The grid's content width, in px. */
  width: number;
  /** The label row under each cover (`labelRowPx`), in px. */
  labelRow: number;
  gap?: number;
  rowGap?: number;
};

/** Equal height: justified rows around `target` px tall. */
export function rowsLayout({ shapes, width, labelRow, target, gap = GAP_PX, rowGap = ROW_GAP_PX }: Common & { target: number }): Layout {
  const rects: Rect[] = new Array(shapes.length);
  const min = Math.max(MIN_COVER_PX, target * ROW_MIN_SCALE);
  const max = target * ROW_MAX_SCALE;
  let y = 0;
  let row: number[] = [];
  let sum = 0;

  /** Lays `row` out at height `h`; `fill` stretches it to the full width. */
  const place = (h: number, fill: boolean) => {
    const height = Math.round(h);
    // Edges are rounded from the exact positions, not from the previous
    // rounded edge, so no card is more than a pixel off its art's shape:
    // carrying the rounding along the row piled it onto the last card.
    let x = 0;
    row.forEach((i, k) => {
      const last = k === row.length - 1;
      const end = x + h * clampShape(shapes[i]);
      const left = Math.round(x);
      const right = fill && last ? width : Math.round(end);
      rects[i] = { x: left, y, w: Math.max(1, right - left), h: height };
      x = end + gap;
    });
    y += height + labelRow + rowGap;
    row = [];
    sum = 0;
  };
  /** The height that makes the row's cards exactly fill the width. */
  const fitted = (count: number, total: number) => (width - gap * (count - 1)) / total;

  shapes.forEach((raw, i) => {
    const shape = clampShape(raw);
    if (row.length === 0) {
      row.push(i);
      sum = shape;
      return;
    }
    const withIt = fitted(row.length + 1, sum + shape);
    if (withIt >= target) {
      // Still too tall to fill the width at the target: keep adding.
      row.push(i);
      sum += shape;
      return;
    }
    const without = fitted(row.length, sum);
    // Break where the row's height lands closest to the target, by ratio,
    // without going under the minimum.
    if (withIt >= min && Math.abs(Math.log(withIt / target)) <= Math.abs(Math.log(without / target))) {
      row.push(i);
      sum += shape;
      place(withIt, true);
      return;
    }
    if (without <= max) place(without, true);
    else place(max, false);
    row.push(i);
    sum = shape;
  });
  if (row.length > 0) {
    // The last row keeps the target height unless its cards would overflow.
    place(Math.min(target, fitted(row.length, sum)), false);
  }
  return { rects, height: Math.max(0, y - rowGap) };
}

/** Equal width: masonry columns at least `minColumn` px wide. */
export function columnsLayout({ shapes, width, labelRow, minColumn, gap = GAP_PX, rowGap = ROW_GAP_PX }: Common & { minColumn: number }): Layout {
  const rects: Rect[] = new Array(shapes.length);
  const count = Math.max(1, Math.floor((width + gap) / (minColumn + gap)));
  const column = (width - gap * (count - 1)) / count;
  const tops: number[] = new Array(count).fill(0);
  const left = (c: number) => Math.round(c * (column + gap));

  shapes.forEach((raw, i) => {
    const shape = clampShape(raw);
    const single = column / shape;
    if (single < MIN_COVER_PX && count >= 2) {
      // Too wide for one column: span two, in the pair whose taller column
      // is shortest.
      let best = 0;
      for (let c = 1; c < count - 1; c++) {
        if (Math.max(tops[c], tops[c + 1]) < Math.max(tops[best], tops[best + 1])) best = c;
      }
      const y = Math.max(tops[best], tops[best + 1]);
      const h = Math.round((2 * column + gap) / shape);
      rects[i] = { x: left(best), y, w: Math.round(2 * column + gap), h };
      tops[best] = tops[best + 1] = y + h + labelRow + rowGap;
      return;
    }
    let c = 0;
    for (let k = 1; k < count; k++) if (tops[k] < tops[c]) c = k;
    const h = Math.round(Math.max(single, MIN_COVER_PX));
    rects[i] = { x: left(c), y: tops[c], w: Math.round(column), h };
    tops[c] += h + labelRow + rowGap;
  });
  return { rects, height: Math.max(0, Math.max(...tops) - rowGap) };
}

/**
 * The first and one-past-last index of the cards whose rectangles (label
 * included) meet `[top, bottom]`. A contiguous range: in Equal width a few
 * cards inside it may be off screen, which costs nothing, and the range
 * changes only when the set at its edges does.
 */
export function visibleRange(layout: Layout, labelRow: number, top: number, bottom: number): { start: number; end: number } {
  let start = -1;
  let end = -1;
  layout.rects.forEach((r, i) => {
    if (r.y <= bottom && r.y + r.h + labelRow >= top) {
      if (start === -1) start = i;
      end = i + 1;
    }
  });
  return start === -1 ? { start: 0, end: 0 } : { start, end };
}

export type NavDirection = 'left' | 'right' | 'up' | 'down';

/**
 * The card arrow-key movement goes to from `index`. Left and right step in
 * reading order. Up and down go to the nearest card above or below that
 * overlaps horizontally — the same column in Equal width, the nearest card of
 * the next row in Equal height — or, when none overlaps, the nearest by
 * distance. At an edge the selection stays.
 */
export function neighbor(rects: readonly Rect[], index: number, dir: NavDirection): number {
  const count = rects.length;
  if (count === 0) return 0;
  const i = Math.min(Math.max(index, 0), count - 1);
  if (dir === 'left') return Math.max(0, i - 1);
  if (dir === 'right') return Math.min(count - 1, i + 1);
  const cur = rects[i];
  const cy = cur.y + cur.h / 2;
  const cx = cur.x + cur.w / 2;
  let best = -1;
  let bestKey: [number, number] = [Infinity, Infinity];
  rects.forEach((r, k) => {
    if (k === i) return;
    const ry = r.y + r.h / 2;
    const dy = dir === 'up' ? cy - ry : ry - cy;
    if (dy <= 1) return;
    const overlap = Math.min(r.x + r.w, cur.x + cur.w) - Math.max(r.x, cur.x);
    // Overlapping cards come first, nearest first; then the rest by distance.
    const key: [number, number] = overlap > 0 ? [dy, -overlap] : [dy + 1e6, Math.abs(r.x + r.w / 2 - cx)];
    if (key[0] < bestKey[0] || (key[0] === bestKey[0] && key[1] < bestKey[1])) {
      best = k;
      bestKey = key;
    }
  });
  return best === -1 ? i : best;
}
