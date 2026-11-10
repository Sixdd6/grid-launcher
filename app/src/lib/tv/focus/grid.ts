// Index moves inside a TV grid (TV mode plan § 1.2 "Windowed grids"). Pure.
// A windowed grid (P7, P8: a 2500-game platform) renders only the covers near
// the view, so the spatial engine can't see the rest: inside the grid, moves
// are index arithmetic, and only leaving it by an edge goes back to the
// spatial engine (`edge`).
import type { Direction } from '../input/buttons';

/**
 * How many columns `repeat(auto-fill, cell)` with a minimum `gap` lays out in
 * `width` (the grid's content box), at least one.
 */
export function gridColumns(width: number, cell: number, gap: number): number {
  if (!(width > 0) || !(cell > 0)) return 1;
  return Math.max(1, Math.floor((width + gap) / (cell + gap)));
}

/**
 * The index a move lands on, or `edge` when it leaves the grid (Up from the
 * top row, Left from the first column, Right from the last, Down from the
 * last row). Down into a shorter last row lands on its last cover, as the
 * spatial rule picks the nearest one below.
 */
export function gridMove(index: number, dir: Direction, columns: number, count: number): number | 'edge' {
  if (count <= 0) return 'edge';
  const cols = Math.max(1, columns);
  const row = Math.floor(index / cols);
  const col = index % cols;
  const lastRow = Math.floor((count - 1) / cols);
  switch (dir) {
    case 'left':
      return col > 0 ? index - 1 : 'edge';
    case 'right':
      return col < cols - 1 && index + 1 < count ? index + 1 : 'edge';
    case 'up':
      return row > 0 ? index - cols : 'edge';
    case 'down':
      if (row >= lastRow) return 'edge';
      return Math.min(index + cols, count - 1);
  }
}

/** Covers a grid renders before it has been measured (its first paint). */
export const INITIAL_WINDOW = 48;

/** Rows rendered past the visible ones on each side, so a held direction
 *  finds the next row already there. */
export const WINDOW_BUFFER_ROWS = 2;

export type WindowInput = {
  count: number;
  columns: number;
  /** One row's height plus the gap under it, in px; 0 before measuring. */
  pitch: number;
  /** The visible band, in px from the top of the grid's first row. */
  top: number;
  bottom: number;
  /** The index the grid holds focus on (kept rendered), or null. */
  focus: number | null;
  buffer?: number;
};

export type GridWindow = {
  /** The first index rendered and one past the last. */
  start: number;
  end: number;
  /** Rows left out above and below the rendered ones. */
  rowsBefore: number;
  rowsAfter: number;
};

/**
 * Which covers a windowed grid renders (risk R8): the rows meeting the
 * visible band, `buffer` rows either side, and the focused cover's row with
 * one row either side of it. When focus sits far from the band (the right
 * stick scrolled away), the rows around focus win: the reveal that follows
 * scrolls them into view. Before measuring, the first `INITIAL_WINDOW`.
 */
export function gridWindow(o: WindowInput): GridWindow {
  const count = Math.max(0, o.count);
  if (count === 0) return { start: 0, end: 0, rowsBefore: 0, rowsAfter: 0 };
  const cols = Math.max(1, o.columns);
  const rows = Math.ceil(count / cols);
  if (!(o.pitch > 0)) {
    const end = Math.min(count, Math.max(INITIAL_WINDOW, o.focus !== null ? (Math.floor(o.focus / cols) + 2) * cols : 0));
    return { start: 0, end, rowsBefore: 0, rowsAfter: rows - Math.ceil(end / cols) };
  }
  const buffer = o.buffer ?? WINDOW_BUFFER_ROWS;
  const last = rows - 1;
  const clamp = (r: number) => Math.max(0, Math.min(last, r));
  let first = clamp(Math.floor(o.top / o.pitch) - buffer);
  let end = clamp(Math.floor(Math.max(o.top, o.bottom) / o.pitch) + buffer);
  if (o.focus !== null && o.focus >= 0 && o.focus < count) {
    const f = Math.floor(o.focus / cols);
    if (f >= first - buffer && f <= end + buffer) {
      first = Math.min(first, clamp(f - 1));
      end = Math.max(end, clamp(f + 1));
    } else {
      const span = end - first;
      first = clamp(f - buffer);
      end = clamp(first + span);
      if (end < f + 1) end = clamp(f + 1);
    }
  }
  return { start: first * cols, end: Math.min(count, (end + 1) * cols), rowsBefore: first, rowsAfter: last - end };
}
