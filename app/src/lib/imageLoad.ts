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
// The windowed grid unmounts the cards it scrolls past, which changes two
// things (`startImageLoad`, `imagePaths`): a slot already granted is held
// until its backend call settles even if its card is gone, and a card that
// comes back renders the file it already resolved without queueing again.
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
  /** Moves a waiting slot between the near and far lanes, to the back of
   *  its new lane. No effect once the slot is granted or released. */
  setPriority: (priority: LoadPriority) => void;
};

export type LoadLimiter = {
  acquire: (priority?: LoadPriority) => LoadSlot;
  /** Slots currently granted, for tests. */
  active: () => number;
  /** Slots waiting, for tests. */
  waiting: () => number;
};

type Entry = { grant: () => void };

/**
 * A two-lane limiter. At most `max` slots are granted at once, and at most
 * `maxFar` of them to far slots. A waiting near slot always goes before a
 * waiting far one; within a lane, slots go in the order they joined it.
 *
 * Each lane is an insertion-ordered `Set`, so joining, leaving, switching
 * lanes and taking the next slot are all constant time: a 2500-game grid
 * switches hundreds of slots per scroll step, and a scan of the whole queue
 * on each switch stalled the page after a scroll.
 */
export function createLoadLimiter(max: number, maxFar: number = max): LoadLimiter {
  let active = 0;
  let activeFar = 0;
  const lanes: Record<LoadPriority, Set<Entry>> = { near: new Set(), far: new Set() };

  function take(lane: Set<Entry>): Entry | undefined {
    const first = lane.values().next();
    if (first.done) return undefined;
    lane.delete(first.value);
    return first.value;
  }

  function pump(): void {
    while (active < max) {
      const entry = take(lanes.near) ?? (activeFar < maxFar ? take(lanes.far) : undefined);
      if (!entry) return;
      entry.grant();
    }
  }

  function acquire(priority: LoadPriority = 'near'): LoadSlot {
    let state: 'waiting' | 'granted' | 'released' = 'waiting';
    let lane = priority;
    let grantedFar = false;
    let resolve!: () => void;
    const ready = new Promise<void>((r) => (resolve = r));
    const entry: Entry = {
      grant: () => {
        state = 'granted';
        grantedFar = lane === 'far';
        active += 1;
        if (grantedFar) activeFar += 1;
        resolve();
      },
    };

    function release(): void {
      if (state === 'released') return;
      if (state === 'waiting') lanes[lane].delete(entry);
      else {
        active -= 1;
        if (grantedFar) activeFar -= 1;
      }
      state = 'released';
      pump();
    }

    function setPriority(next: LoadPriority): void {
      if (state !== 'waiting' || lane === next) return;
      lanes[lane].delete(entry);
      lane = next;
      lanes[lane].add(entry);
      pump();
    }

    lanes[lane].add(entry);
    pump();
    return { ready, release, setPriority };
  }

  return {
    acquire,
    active: () => active,
    waiting: () => lanes.near.size + lanes.far.size,
  };
}

/** The one limiter every `Image.svelte` shares. */
export const imageLoads = createLoadLimiter(MAX_CONCURRENT_IMAGE_LOADS, MAX_FAR_IMAGE_LOADS);

/** About forty screens of covers at the default size: far more than one
 *  session scrolls back to, and a few hundred kilobytes at most. */
export const IMAGE_PATH_MEMO_SIZE = 4096;

export type PathMemo = {
  get: (url: string) => string | undefined;
  set: (url: string, path: string) => void;
  delete: (url: string) => void;
  size: () => number;
};

/**
 * Which local file each image url resolved to, so a card that remounts (the
 * windowed grid unmounts the rows it scrolls past) renders its `<img>` at
 * once instead of waiting for a slot and an IPC call it already made. At
 * most `limit` entries; the oldest goes first.
 */
export function createPathMemo(limit: number): PathMemo {
  const paths = new Map<string, string>();
  return {
    get: (url) => paths.get(url),
    set: (url, path) => {
      paths.delete(url);
      paths.set(url, path);
      while (paths.size > limit) {
        const oldest = paths.keys().next();
        if (oldest.done) break;
        paths.delete(oldest.value);
      }
    },
    delete: (url) => void paths.delete(url),
    size: () => paths.size,
  };
}

/** The memo every `Image.svelte` shares. */
export const imagePaths = createPathMemo(IMAGE_PATH_MEMO_SIZE);


export type ImageLoad = {
  /** This load's slot, for its priority while it waits. */
  slot: LoadSlot;
  /** The local file, once the slot is granted and the backend has it.
   *  Never settles for a load cancelled before its slot was granted. */
  path: Promise<string>;
  /** The caller is done with the image: the `<img>` loaded or failed. */
  done: () => void;
  /** The caller went away. A waiting slot leaves the queue at once; a
   *  granted one is held until its backend call settles. */
  cancel: () => void;
};

/**
 * One image's load: wait for a slot, ask the backend for the file, and
 * remember where it is.
 *
 * The slot is NOT freed while the backend call is still running, even if
 * the caller cancels. The backend keeps fetching a cover whether or not
 * anyone still wants it, so freeing the slot early would let the next image
 * start another fetch on top of it: a fast scroll through a windowed grid
 * unmounts dozens of cards a second, and the backend's queue would grow
 * without bound (2026-10-20).
 */
export function startImageLoad(
  url: string,
  ensure: (url: string) => Promise<string>,
  limiter: LoadLimiter = imageLoads,
  memo: PathMemo = imagePaths
): ImageLoad {
  const slot = limiter.acquire('far');
  let cancelled = false;
  let inFlight = false;
  const path = slot.ready.then(() => {
    if (cancelled) return new Promise<string>(() => {});
    inFlight = true;
    return ensure(url).then(
      (resolved) => {
        inFlight = false;
        memo.set(url, resolved);
        if (cancelled) slot.release();
        return resolved;
      },
      (error: unknown) => {
        inFlight = false;
        slot.release();
        throw error;
      }
    );
  });
  // A cancelled load's rejection has no one to report to.
  path.catch(() => {});
  return {
    slot,
    path,
    done: () => slot.release(),
    cancel: () => {
      cancelled = true;
      if (!inFlight) slot.release();
    },
  };
}

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
