// Warms a card's background art as the card scrolls into view, so the first
// hover of a game the user has never opened does not pay for the download,
// decode and blur (design §3). The grid is watched with one
// `IntersectionObserver`; the queue and the memo live in
// `backgroundPrefetch.ts`, which is the only module that talks to the
// backend about variants.
import type { BackgroundSubject } from './background';
import { warmBackground } from './backgroundPrefetch';

/** One row ahead. Cards are 200px-ish tall at every size, so a card entering
 *  this margin is about to be scrolled onto, not merely near. */
export const WARM_ROOT_MARGIN = '200px';

export type VisibleWarmer = {
  /** Observes every current child of `grid`. Call again after the list
   *  changes: a refresh or a filter adds children this has never seen. */
  observe: (grid: HTMLElement) => void;
  /** Stops watching everything. Call from the view's effect teardown. */
  disconnect: () => void;
};

/**
 * A warmer for one grid. `subjectAt` maps a child's position in the grid to
 * the game it renders — the view owns the list, so it owns that lookup — and
 * returns `null` when the index no longer names a row.
 *
 * Warming is once per element: the entry's target is unobserved as soon as
 * it has been warmed, so a card that scrolls in and out again costs nothing.
 * `warmBackground` de-duplicates a second time by URL, which covers the same
 * game appearing in two views.
 */
export function createVisibleWarmer(
  subjectAt: (index: number) => BackgroundSubject | null
): VisibleWarmer {
  // No `IntersectionObserver` under SSR or in the node test runner. The
  // webview has one, so this branch never runs in the app — it exists so
  // mounting the warmer cannot throw where the API is missing.
  if (typeof IntersectionObserver === 'undefined') {
    return { observe: () => {}, disconnect: () => {} };
  }

  let observer: IntersectionObserver | null = null;
  // The grid the entries' targets are children of, for the index lookup.
  // Held rather than re-passed: the callback is the browser's to call.
  let watched: HTMLElement | null = null;

  function onEntries(entries: IntersectionObserverEntry[]): void {
    const grid = watched;
    if (grid === null) return;
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const index = Array.prototype.indexOf.call(grid.children, entry.target);
      if (index < 0) continue;
      const subject = subjectAt(index);
      if (subject === null) continue;
      warmBackground(subject);
      observer?.unobserve(entry.target);
    }
  }

  function observe(grid: HTMLElement): void {
    watched = grid;
    observer ??= new IntersectionObserver(onEntries, { rootMargin: WARM_ROOT_MARGIN });
    // `observe` on an element already being watched is a no-op per spec, so
    // re-running this after a refresh only picks up the new children.
    for (const child of Array.from(grid.children)) observer.observe(child);
  }

  function disconnect(): void {
    observer?.disconnect();
    observer = null;
    watched = null;
  }

  return { observe, disconnect };
}
