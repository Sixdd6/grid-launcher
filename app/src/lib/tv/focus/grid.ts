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
