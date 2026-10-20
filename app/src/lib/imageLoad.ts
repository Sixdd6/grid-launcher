// Keeps the number of images loading at once bounded, however long the grid,
// and serves the ones the user can see first.
//
// Every `Image.svelte` used to call `ensure_image` and mount its `<img>` the
// moment it mounted. On a 2000-game platform that is 2000 IPC calls and 2000
// asset-protocol requests in flight together, and the web process ran out of
// file descriptors ("Creating pipes for GWakeup: Too many open files") and
// aborted, leaving the window an empty shell (2026-10-20).
//
// Now every image takes a slot from `imageLoads` and holds it from "asked the
// backend for the file" until "the `<img>` has loaded or failed": at most
// `MAX_CONCURRENT_IMAGE_LOADS` at once. A waiting image is `near` while it is
// within `IMAGE_NEAR_MARGIN` of its scroll container's visible area
// (`watchNear`) and `far` otherwise, and the queue serves every near image
// before any far one. Far images still load, at most `MAX_FAR_IMAGE_LOADS` at
// a time in the order they mounted, so a platform keeps filling in behind the
// user the way it did when every cover loaded at open.
//
// Near is re-decided as the user scrolls. A first version only queued an
// image once it came near and never let go of it, so after scrolling through
// a platform the cards on screen waited behind every card passed on the way
// (2026-10-20, the `large-platform` e2e group).
//
// Not the browser's `loading="lazy"`: that waits until the image is ON
// screen, and the first scroll through a fresh platform paid for every
// fetch and decode mid-scroll (see the note in Image.svelte).

import { scrollParent } from './visibleWarm';

/** Two to three rows of cards at the default size, above and below. */
export const IMAGE_NEAR_MARGIN = '600px 0px';

/** Enough to fill the visible rows quickly, far below any file-descriptor
 *  limit. */
export const MAX_CONCURRENT_IMAGE_LOADS = 12;

/** Background loads of images nowhere near the screen. Few, so they never
 *  take more than a third of the backend's six download permits from the
 *  images the user is looking at. */
export const MAX_FAR_IMAGE_LOADS = 2;

export type LoadPriority = 'near' | 'far';

export type LoadSlot = {
  /** Resolves when the slot is granted. Never resolves for a slot released
   *  before it was granted. */
  ready: Promise<void>;
  /** Frees the slot, or withdraws it from the queue if it was not granted
   *  yet. Safe to call more than once. */
  release: () => void;
  /** Moves a waiting slot between the near and far lanes. No effect once
   *  the slot is granted or released. */
  setPriority: (priority: LoadPriority) => void;
};

export type LoadLimiter = {
  acquire: (priority?: LoadPriority) => LoadSlot;
  /** Slots currently granted, for tests. */
  active: () => number;
  /** Slots waiting, for tests. */
  waiting: () => number;
};

type Entry = { priority: LoadPriority; grant: () => void };

/**
 * A two-lane limiter. At most `max` slots are granted at once, and at most
 * `maxFar` of them to far slots. A waiting near slot always goes before a
 * waiting far one; within a lane, slots go in the order they were acquired.
 */
export function createLoadLimiter(max: number, maxFar: number = max): LoadLimiter {
  let active = 0;
  let activeFar = 0;
  const queue: Entry[] = [];

  function pump(): void {
    while (active < max) {
      let i = queue.findIndex((e) => e.priority === 'near');
      if (i === -1 && activeFar < maxFar) i = queue.findIndex((e) => e.priority === 'far');
      if (i === -1) return;
      const [entry] = queue.splice(i, 1);
      entry.grant();
    }
  }

  function acquire(priority: LoadPriority = 'near'): LoadSlot {
    let state: 'waiting' | 'granted' | 'released' = 'waiting';
    let grantedFar = false;
    let resolve!: () => void;
    const ready = new Promise<void>((r) => (resolve = r));
    const entry: Entry = {
      priority,
      grant: () => {
        state = 'granted';
        grantedFar = entry.priority === 'far';
        active += 1;
        if (grantedFar) activeFar += 1;
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
        if (grantedFar) activeFar -= 1;
      }
      state = 'released';
      pump();
    }

    function setPriority(next: LoadPriority): void {
      if (state !== 'waiting' || entry.priority === next) return;
      entry.priority = next;
      pump();
    }

    queue.push(entry);
    pump();
    return { ready, release, setPriority };
  }

  return { acquire, active: () => active, waiting: () => queue.length };
}

/** The one limiter every `Image.svelte` shares. */
export const imageLoads = createLoadLimiter(MAX_CONCURRENT_IMAGE_LOADS, MAX_FAR_IMAGE_LOADS);

type Watch = {
  observer: IntersectionObserver;
  callbacks: Map<Element, (near: boolean) => void>;
};

// One observer per scroll container: `rootMargin` only expands the root's
// own rect (see `scrollParent`), so each container needs its own root.
const watches = new Map<Element | null, Watch>();

function watchFor(root: Element | null): Watch {
  let watch = watches.get(root);
  if (watch) return watch;
  const callbacks = new Map<Element, (near: boolean) => void>();
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) callbacks.get(entry.target)?.(entry.isIntersecting);
    },
    { root, rootMargin: IMAGE_NEAR_MARGIN }
  );
  watch = { observer, callbacks };
  watches.set(root, watch);
  return watch;
}

/**
 * Calls `onChange(true)` whenever `el` comes within `IMAGE_NEAR_MARGIN` of
 * its scroll container's visible area and `onChange(false)` whenever it
 * leaves. Returns a function that stops watching.
 *
 * Without `IntersectionObserver` (SSR, the node test runner) it reports near
 * once, at once: there is nothing to watch, and treating every image as far
 * would only slow it down.
 */
export function watchNear(el: Element, onChange: (near: boolean) => void): () => void {
  if (typeof IntersectionObserver === 'undefined') {
    onChange(true);
    return () => {};
  }
  const watch = watchFor(scrollParent(el));
  watch.callbacks.set(el, onChange);
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
