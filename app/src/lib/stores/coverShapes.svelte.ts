// The covers each grid has shown, by shape (`cards/shape.ts`). Module scoped
// and kept for the session, so a platform opened again starts at the cell
// shape it settled on instead of shifting a second time. Not persisted: the
// first open of a session measures again.
import { SvelteMap } from 'svelte/reactivity';
import { cellAspect, MAX_SAMPLES } from '../cards/shape';

const samples = new Map<string, number[]>();
/** The settled ratio per grid, reactive so the grid re-lays out once. */
const aspects = new SvelteMap<string, number>();

/** Records one loaded cover's natural size for the grid `key`. */
export function recordCover(key: string, width: number, height: number): void {
  if (!(width > 0 && height > 0)) return;
  const list = samples.get(key) ?? [];
  if (list.length >= MAX_SAMPLES) return;
  list.push(width / height);
  samples.set(key, list);
  const next = cellAspect(list);
  if (aspects.get(key) !== next) aspects.set(key, next);
}

/** The cell ratio (width over height) for the grid `key`. */
export function cellAspectFor(key: string): number {
  return aspects.get(key) ?? cellAspect([]);
}
