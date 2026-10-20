// Which cards a grid renders (the windowed `CardGrid`). Pure: the component
// measures the grid and its scroll container, this module turns those
// numbers into a range of cards and the padding that stands in for the rest.
//
// Why a window at all: with every card of a 2000-game platform in the DOM,
// WebKitGTK's style, layout and paint took 200+ ms per frame for both scroll
// and hover (measured 2026-10-20). The cost is the number of cards in the
// DOM, so only the rows near the visible part of the scroll container exist.
// A platform that fits on screen renders every card, exactly as before.
import { CARD_COVER_ASPECT, CARD_GAP_PX, TITLE_ROW_HEIGHT_PX } from './size';

/** The grid's `gap`, in px. `CardGrid.svelte` sets it from this. */
export const GRID_GAP_PX = 16;

/** Rows rendered above and below the visible part. Keyboard movement goes a
 *  row at a time, so the next row is always already rendered. */
export const BUFFER_ROWS = 2;

/** Cards rendered before the grid has been measured: the first paint, and
 *  server-side rendering. Divisible by every column count up to 4 and by 6,
 *  8 and 12, so the last rendered row is full. */
export const INITIAL_WINDOW_CARDS = 96;

export type RowMetrics = {
  /** One card's height: the cover, the gap under it and the title strip. */
  rowHeight: number;
  /** From one row's top to the next row's top: the row plus the gap. */
  pitch: number;
};

export type WindowRange = {
  /** The first rendered card's index. */
  start: number;
  /** One past the last rendered card's index. */
  end: number;
  /** The height of the rows above the window, in px. */
  padTop: number;
  /** The height of the rows below the window, in px. */
  padBottom: number;
};

/**
 * A card's row height and pitch for a column `trackPx` wide. Matches the
 * card's own layout: a 3:4 cover, then `CARD_GAP_PX`, then the title.
 *
 * Rounded to whole px, and the grid lays its rows out at exactly this height
 * (`grid-auto-rows`). WebKit rounds a fractional row to 1/64 px, which over
 * the 800 rows of a 2500-game platform put the math 4px off the layout.
 */
export function rowMetrics(trackPx: number, gap: number = GRID_GAP_PX): RowMetrics {
  const rowHeight = Math.round(trackPx / CARD_COVER_ASPECT + CARD_GAP_PX + TITLE_ROW_HEIGHT_PX);
  return { rowHeight, pitch: rowHeight + gap };
}

/** The range before any measurement: the first `INITIAL_WINDOW_CARDS`. */
export function initialRange(count: number): WindowRange {
  return { start: 0, end: Math.min(Math.max(0, count), INITIAL_WINDOW_CARDS), padTop: 0, padBottom: 0 };
}

/**
 * The cards to render for one scroll position.
 *
 * `offset` is how far the scroll container's visible top is below the top of
 * the grid's first row, in px: negative while the grid starts lower down the
 * screen. The window covers every row that overlaps the visible part, plus
 * `bufferRows` either side. The rows outside it become `padTop` and
 * `padBottom`, so the grid's height, and so the scrollbar, match the whole
 * list.
 */
export function windowRange({
  offset,
  viewportHeight,
  pitch,
  columns,
  count,
  bufferRows = BUFFER_ROWS,
}: {
  offset: number;
  viewportHeight: number;
  pitch: number;
  columns: number;
  count: number;
  bufferRows?: number;
}): WindowRange {
  if (count <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  if (!(pitch > 0) || !(columns >= 1) || !(viewportHeight > 0)) return initialRange(count);
  const cols = Math.floor(columns);
  const totalRows = Math.ceil(count / cols);
  const lastRow = Math.min(
    totalRows,
    Math.max(0, Math.ceil((offset + viewportHeight) / pitch)) + bufferRows
  );
  // Never past the last row, so a stale offset beyond the end (the list just
  // shrank) still renders the end of the list rather than nothing.
  const firstRow = Math.min(
    Math.max(0, Math.floor(Math.max(0, offset) / pitch) - bufferRows),
    Math.max(0, lastRow - 1)
  );
  return {
    start: firstRow * cols,
    end: Math.min(count, lastRow * cols),
    padTop: firstRow * pitch,
    padBottom: (totalRows - lastRow) * pitch,
  };
}

/**
 * The scroll position that brings card `index` into view with the least
 * movement, like `scrollIntoView({ block: 'nearest' })`, for a card that is
 * not rendered and so cannot be scrolled to directly.
 *
 * `rowsTop` is where the grid's first row starts in the scroll container's
 * content, in px. Returns `scrollTop` itself when the card's row is already
 * fully visible.
 */
export function scrollTopForIndex({
  index,
  columns,
  metrics,
  rowsTop,
  scrollTop,
  viewportHeight,
}: {
  index: number;
  columns: number;
  metrics: RowMetrics;
  rowsTop: number;
  scrollTop: number;
  viewportHeight: number;
}): number {
  const row = Math.floor(Math.max(0, index) / Math.max(1, Math.floor(columns)));
  const top = rowsTop + row * metrics.pitch;
  const bottom = top + metrics.rowHeight;
  if (top < scrollTop) return top;
  if (bottom > scrollTop + viewportHeight) return Math.min(top, bottom - viewportHeight);
  return scrollTop;
}
