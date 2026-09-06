// Warms a card's background art as the card scrolls into view, so the first
// hover of a game the user has never opened does not pay for the download,
// decode and blur (design §3). The grid is watched with one
// `IntersectionObserver`; the queue and the memo live in
// `backgroundPrefetch.ts`, which is the only module that talks to the
// backend about variants.
import type { BackgroundSubject } from './background';
import { warmBackground } from './backgroundPrefetch';

/** One row ahead, vertically only: the grid never scrolls sideways, and a
 *  horizontal margin would pull in the cards either side of the viewport for
 *  nothing. Expands the SCROLL CONTAINER's rect, not the viewport's — see
 *  `scrollParent`. */
export const WARM_ROOT_MARGIN = '200px 0px';

/**
 * The nearest ancestor of `el` that actually scrolls, or `null` when nothing
 * between `el` and the document does.
 *
 * `IntersectionObserver` applies every clipping ancestor's rect between the
 * target and the root WITHOUT `rootMargin` — only the root's own rect is
 * expanded. The grid scrolls inside the view's `.body` (`overflow-y: auto`),
 * not inside the viewport, so observing against the default root would clip
 * each card at `.body`'s edge and the 200px lookahead would buy nothing:
 * cards would warm as they became visible, not a row early.
 */
export function scrollParent(el: Element): Element | null {
  // A truthy test, not `!== null`: `parentElement` is `null` at the document
  // root in a browser, but a plain stand-in object may simply not have it.
  for (let node = el.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === 'auto' || overflow === 'scroll') return node;
  }
  return null;
}

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
      // Stop watching only once the card is DEALT WITH. With the background
      // art switched off nothing is built, so the card stays observed and
      // warms for real if the setting comes back.
      if (!warmBackground(subject)) continue;
      observer?.unobserve(entry.target);
    }
  }

  function observe(grid: HTMLElement): void {
    watched = grid;
    // `null` means the viewport, which is the right answer when nothing
    // between the grid and the document scrolls.
    observer ??= new IntersectionObserver(onEntries, {
      root: scrollParent(grid),
      rootMargin: WARM_ROOT_MARGIN,
    });
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
