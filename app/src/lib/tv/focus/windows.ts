// Windowed TV grids (risk R8; TV mode plan § 1.2 "Windowed grids"). A grid
// that renders only the covers near the view (`ui/TvGrid.svelte`) registers
// a handle here, on its `[data-window]` element, so the focus code can move
// inside it on the layout (`grid.ts` `layoutMove`), bring an unrendered cover into
// the DOM before focusing it, and find a remembered cover by its key.
// Each cover sits in a `[data-i]` cell (its index in the whole list).
import type { Direction } from '../input/buttons';

export type WindowHandle = {
  /** Covers in the whole list. */
  count(): number;
  /** Where a move from cover `i` lands, on the layout as it is now, or
   *  `edge` when it leaves the grid. */
  move(i: number, dir: Direction): number | 'edge';
  /** The index of the cover with `data-k` `k`, or -1. */
  indexOfKey(k: string): number;
  /** Renders cover `i` (synchronously) and answers its focusable. */
  ensure(i: number): HTMLElement | null;
  /** Focus is on cover `i`: keep its row rendered. */
  focusAt(i: number): void;
  /** Re-reads the scroll position and renders the covers it shows. */
  sync(): void;
};

const handles = new WeakMap<Element, WindowHandle>();

export function registerWindow(el: Element, h: WindowHandle): () => void {
  handles.set(el, h);
  return () => {
    if (handles.get(el) === h) handles.delete(el);
  };
}

/** The windowed grid around `el` and `el`'s index in it, or null. */
export function windowOf(el: Element | null): { handle: WindowHandle; index: number } | null {
  const cell = el?.closest<HTMLElement>('[data-i]');
  const grid = cell?.closest('[data-window]');
  const handle = grid ? handles.get(grid) : undefined;
  if (!cell || !handle) return null;
  const index = Number(cell.getAttribute('data-i'));
  return Number.isInteger(index) ? { handle, index } : null;
}

/** Every windowed grid's handle inside `root`. */
export function windowsIn(root: Element | null): WindowHandle[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll('[data-window]')).flatMap((g) => {
    const h = handles.get(g);
    return h ? [h] : [];
  });
}

/** Renders the cover with key `k` in any windowed grid inside `root`. */
export function materialize(root: Element | null, k: string): HTMLElement | null {
  for (const h of windowsIn(root)) {
    const i = h.indexOfKey(k);
    if (i >= 0) return h.ensure(i);
  }
  return null;
}
