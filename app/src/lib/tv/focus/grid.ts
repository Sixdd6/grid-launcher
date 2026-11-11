// Moves inside a TV grid (TV mode plan § 1.2 "Windowed grids"). Pure.
// A windowed grid (P7, P8: a 2500-game platform) renders only the covers near
// the view, so the spatial engine can't see the rest. Every cover's rectangle
// is known from the layout (`cards/layout.ts`, the desktop's: covers sized by
// their art, in Equal height rows or Equal width columns), so moves inside
// the grid are worked out on those rectangles, and only leaving it by an
// edge goes back to the spatial engine (`edge`).
import type { Rect } from '../../cards/layout';
import type { Direction } from '../input/buttons';

/**
 * The index a move from `index` lands on, or `edge` when it leaves the grid.
 *
 * - Left and Right go to the nearest cover beside this one that shares some
 *   of its height: the next one in the row (Equal height), or in the next
 *   column (Equal width). With none, the move leaves the grid.
 * - Up and Down go to the nearest cover above or below that shares some of
 *   its width, the most shared first; when none does (Down into a shorter
 *   last row), to the nearest one by distance. With none, it leaves.
 */
export function layoutMove(rects: readonly Rect[], index: number, dir: Direction): number | 'edge' {
  const cur = rects[index];
  if (!cur) return 'edge';
  const sideways = dir === 'left' || dir === 'right';
  const cx = cur.x + cur.w / 2;
  const cy = cur.y + cur.h / 2;
  let best = -1;
  let bestKey: [number, number] = [Infinity, Infinity];
  rects.forEach((r, k) => {
    if (k === index || !r) return;
    const rx = r.x + r.w / 2;
    const ry = r.y + r.h / 2;
    const d = dir === 'left' ? cx - rx : dir === 'right' ? rx - cx : dir === 'up' ? cy - ry : ry - cy;
    if (d <= 1) return;
    const overlap = sideways
      ? Math.min(r.y + r.h, cur.y + cur.h) - Math.max(r.y, cur.y)
      : Math.min(r.x + r.w, cur.x + cur.w) - Math.max(r.x, cur.x);
    if (overlap <= 0 && sideways) return;
    const key: [number, number] = overlap > 0 ? [d, -overlap] : [d + 1e6, Math.abs(rx - cx)];
    if (key[0] < bestKey[0] || (key[0] === bestKey[0] && key[1] < bestKey[1])) {
      best = k;
      bestKey = key;
    }
  });
  return best === -1 ? 'edge' : best;
}

/** Covers a grid renders before it has been measured (its first paint). */
export const INITIAL_WINDOW = 48;

/**
 * The indexes a windowed grid renders: the contiguous run whose rectangles
 * meet the visible band (`range`, from `visibleRange`), plus the focused
 * cover when it sits outside it (the right stick scrolled away), so focus
 * never points at a cover that is not in the DOM. Ascending.
 */
export function windowIndexes(range: { start: number; end: number }, focus: number | null, count: number): number[] {
  const out: number[] = [];
  const end = Math.min(range.end, count);
  if (focus !== null && focus >= 0 && focus < count && focus < range.start) out.push(focus);
  for (let i = Math.max(0, range.start); i < end; i++) out.push(i);
  if (focus !== null && focus >= 0 && focus < count && focus >= end) out.push(focus);
  return out;
}
