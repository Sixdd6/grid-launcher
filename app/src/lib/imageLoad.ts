// Keeps the number of images loading at once bounded, however long the grid.
//
// Every `Image.svelte` used to call `ensure_image` and mount its `<img>` the
// moment it mounted. On a 2000-game platform that is 2000 IPC calls and 2000
// asset-protocol requests in flight together, and the web process ran out of
// file descriptors ("Creating pipes for GWakeup: Too many open files") and
// aborted, leaving the window an empty shell (2026-10-20). Two bounds fix it:
//
// - `whenNear`: an image starts loading only once it is within
//   `IMAGE_NEAR_MARGIN` of its scroll container's visible area. A card on a
//   hidden view (`display: none`) is never near, so it never loads.
// - `imageLoads`: at most `MAX_CONCURRENT_IMAGE_LOADS` images between "asked
//   the backend for the file" and "the `<img>` has loaded or failed".
//
// Not the browser's `loading="lazy"`: that waits until the image is ON
// screen, and the first scroll through a fresh platform paid for every
// fetch and decode mid-scroll (see the note in Image.svelte). Starting a
// couple of rows early keeps the eager-decode behaviour that note measured.

import { scrollParent } from './visibleWarm';

/** Two to three rows of cards at the default size, above and below. */
export const IMAGE_NEAR_MARGIN = '600px 0px';

/** Enough to fill the visible rows quickly from the local cache, far below
 *  any file-descriptor limit. */
export const MAX_CONCURRENT_IMAGE_LOADS = 12;

export type LoadSlot = {
  /** Resolves when the slot is granted. Never resolves for a slot released
   *  before it was granted. */
  ready: Promise<void>;
  /** Frees the slot, or withdraws it from the queue if it was not granted
   *  yet. Safe to call more than once. */
  release: () => void;
};

export type LoadLimiter = {
  acquire: () => LoadSlot;
  /** Slots currently granted, for tests. */
  active: () => number;
  /** Slots waiting, for tests. */
  waiting: () => number;
};

/** A FIFO limiter: the first `max` acquires are granted at once, the rest
 *  in order as earlier slots are released. */
export function createLoadLimiter(max: number): LoadLimiter {
  let active = 0;
  const queue: { grant: () => void }[] = [];

  function acquire(): LoadSlot {
    let state: 'waiting' | 'granted' | 'released' = 'waiting';
    let resolve!: () => void;
    const ready = new Promise<void>((r) => (resolve = r));
    const entry = {
      grant: () => {
        state = 'granted';
        active += 1;
        resolve();
      },
    };

    function release(): void {
      if (state === 'released') return;
      if (state === 'waiting') {
        const i = queue.indexOf(entry);
        if (i !== -1) queue.splice(i, 1);
      } else {
        active -= 1;
        queue.shift()?.grant();
      }
      state = 'released';
    }

    if (active < max) entry.grant();
    else queue.push(entry);
    return { ready, release };
  }

  return { acquire, active: () => active, waiting: () => queue.length };
}

/** The one limiter every `Image.svelte` shares. */
export const imageLoads = createLoadLimiter(MAX_CONCURRENT_IMAGE_LOADS);

type Watch = { observer: IntersectionObserver; callbacks: Map<Element, () => void> };

// One observer per scroll container: `rootMargin` only expands the root's
// own rect (see `scrollParent`), so each container needs its own root.
const watches = new Map<Element | null, Watch>();

function watchFor(root: Element | null): Watch {
  let watch = watches.get(root);
  if (watch) return watch;
  const callbacks = new Map<Element, () => void>();
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const callback = callbacks.get(entry.target);
        if (!callback) continue;
        callbacks.delete(entry.target);
        observer.unobserve(entry.target);
        callback();
      }
    },
    { root, rootMargin: IMAGE_NEAR_MARGIN }
  );
  watch = { observer, callbacks };
  watches.set(root, watch);
  return watch;
}

/**
 * Calls `onNear` once, when `el` comes within `IMAGE_NEAR_MARGIN` of its
 * scroll container's visible area. Returns a function that stops watching.
 *
 * Without `IntersectionObserver` (SSR, the node test runner) `onNear` runs at
 * once: there is nothing to wait for, and holding the image back forever
 * would be worse than loading it.
 */
export function whenNear(el: Element, onNear: () => void): () => void {
  if (typeof IntersectionObserver === 'undefined') {
    onNear();
    return () => {};
  }
  const root = scrollParent(el);
  const watch = watchFor(root);
  watch.callbacks.set(el, onNear);
  watch.observer.observe(el);
  return () => {
    if (!watch.callbacks.delete(el)) return;
    watch.observer.unobserve(el);
  };
}

/** Test seam: forget every observer. */
export function resetNearWatchesForTest(): void {
  for (const watch of watches.values()) watch.observer.disconnect();
  watches.clear();
}
