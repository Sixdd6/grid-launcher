// The shape (width over height) of every cover GRID has shown, by cover URL
// (`cards/layout.ts`). A cover's shape is only known once it has loaded;
// until then its card is portrait, and the grid re-lays out when the image
// arrives. Remembering shapes across sessions means a platform seen before
// lays out final at once, with nothing shifting under the user.
//
// Persisted in localStorage as one JSON object, capped at `MAX_SHAPES`
// (the oldest dropped first), and written at most once a second.
import { SvelteMap } from 'svelte/reactivity';

const STORAGE_KEY = 'grid.coverShapes.v1';
export const MAX_SHAPES = 20000;
const SAVE_DELAY_MS = 1000;

const shapes = new SvelteMap<string, number>(load());
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function load(): [string, number][] {
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    return Object.entries(parsed).filter((e): e is [string, number] => typeof e[1] === 'number' && e[1] > 0);
  } catch {
    // No storage, or a damaged entry: start empty; shapes come back as
    // covers load.
    return [];
  }
}

function scheduleSave(): void {
  if (saveTimer !== null) return;
  saveTimer = setTimeout(() => {
    saveTimer = null;
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(Object.fromEntries(shapes)));
    } catch {
      // Storage full or blocked: the shapes stay for this session.
    }
  }, SAVE_DELAY_MS);
}

/** Records a loaded cover's natural size. Rounded to 3 places, so a cover
 *  that loads again at another scale does not re-lay out its grid. */
export function recordShape(url: string | null, width: number, height: number): void {
  if (!url || !(width > 0 && height > 0)) return;
  const shape = Math.round((width / height) * 1000) / 1000;
  if (shapes.get(url) === shape) return;
  shapes.delete(url);
  shapes.set(url, shape);
  while (shapes.size > MAX_SHAPES) shapes.delete(shapes.keys().next().value as string);
  scheduleSave();
}

/** The cover's shape, or `undefined` while it is not known. Reactive. */
export function shapeOf(url: string | null): number | undefined {
  return url ? shapes.get(url) : undefined;
}
