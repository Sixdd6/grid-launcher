import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createLoadLimiter,
  createPathMemo,
  IMAGE_NEAR_MARGIN,
  IMAGE_PATH_MEMO_SIZE,
  MAX_CONCURRENT_IMAGE_LOADS,
  MAX_FAR_IMAGE_LOADS,
  resetNearWatchesForTest,
  startImageLoad,
  watchNear,
} from './imageLoad';

/** Lets every resolved promise's callbacks run. */
const flush = () => new Promise<void>((r) => setTimeout(r, 0));

describe('createLoadLimiter', () => {
  it('grants up to the limit at once and queues the rest', async () => {
    const limiter = createLoadLimiter(2);
    const granted: number[] = [];
    const slots = [0, 1, 2, 3].map((i) => {
      const slot = limiter.acquire();
      void slot.ready.then(() => granted.push(i));
      return slot;
    });
    await flush();
    expect(granted).toEqual([0, 1]);
    expect(limiter.active()).toBe(2);
    expect(limiter.waiting()).toBe(2);

    slots[0].release();
    await flush();
    expect(granted).toEqual([0, 1, 2]);
    expect(limiter.active()).toBe(2);
    expect(limiter.waiting()).toBe(1);
  });

  it('withdraws a slot released before it was granted', async () => {
    const limiter = createLoadLimiter(1);
    const first = limiter.acquire();
    const second = limiter.acquire();
    const third = limiter.acquire();
    let thirdGranted = false;
    void third.ready.then(() => (thirdGranted = true));

    second.release();
    expect(limiter.waiting()).toBe(1);
    first.release();
    await flush();
    expect(thirdGranted).toBe(true);
    expect(limiter.active()).toBe(1);
  });

  it('ignores a second release', async () => {
    const limiter = createLoadLimiter(1);
    const first = limiter.acquire();
    limiter.acquire();
    first.release();
    first.release();
    await flush();
    expect(limiter.active()).toBe(1);
    expect(limiter.waiting()).toBe(0);
  });

  it('keeps the shared limit well under a file-descriptor limit', () => {
    expect(MAX_CONCURRENT_IMAGE_LOADS).toBeGreaterThan(0);
    expect(MAX_CONCURRENT_IMAGE_LOADS).toBeLessThanOrEqual(32);
    expect(MAX_FAR_IMAGE_LOADS).toBeLessThan(MAX_CONCURRENT_IMAGE_LOADS);
  });
});

describe('createLoadLimiter lanes', () => {
  /** Acquires `priority` slots tagged with `name`, recording grant order. */
  function track(limiter: ReturnType<typeof createLoadLimiter>, granted: string[]) {
    return (name: string, priority: 'near' | 'far') => {
      const slot = limiter.acquire(priority);
      void slot.ready.then(() => granted.push(name));
      return slot;
    };
  }

  it('serves every waiting near slot before any far one', async () => {
    const limiter = createLoadLimiter(1, 1);
    const granted: string[] = [];
    const take = track(limiter, granted);
    const first = take('first', 'near');
    take('far-a', 'far');
    take('near-b', 'near');
    first.release();
    await flush();
    expect(granted).toEqual(['first', 'near-b']);
  });

  it('grants at most maxFar far slots, leaving the rest for near ones', async () => {
    const limiter = createLoadLimiter(4, 2);
    const granted: string[] = [];
    const take = track(limiter, granted);
    ['f1', 'f2', 'f3'].forEach((n) => take(n, 'far'));
    await flush();
    expect(granted).toEqual(['f1', 'f2']);
    take('n1', 'near');
    take('n2', 'near');
    await flush();
    expect(granted).toEqual(['f1', 'f2', 'n1', 'n2']);
    expect(limiter.waiting()).toBe(1);
  });

  it('moves a waiting slot between lanes as it comes near and leaves', async () => {
    const limiter = createLoadLimiter(1, 0);
    const granted: string[] = [];
    const take = track(limiter, granted);
    const blocker = take('blocker', 'near');
    const passed = take('passed', 'near');
    const onScreen = take('on-screen', 'near');
    // Scrolled past before its turn: it drops behind what is on screen.
    passed.setPriority('far');
    blocker.release();
    await flush();
    expect(granted).toEqual(['blocker', 'on-screen']);

    // Back in view: near again, and next once a slot frees (no far lane
    // here, so as a far slot it would never be served).
    passed.setPriority('near');
    onScreen.release();
    await flush();
    expect(granted).toEqual(['blocker', 'on-screen', 'passed']);
  });

  it('ignores a priority change once granted', async () => {
    const limiter = createLoadLimiter(1, 1);
    const slot = limiter.acquire('far');
    slot.setPriority('near');
    limiter.acquire('far');
    slot.release();
    expect(limiter.active()).toBe(1);
  });
});

describe('createPathMemo', () => {
  it('remembers, forgets and evicts the oldest entry past its limit', () => {
    const memo = createPathMemo(2);
    memo.set('a', '/a');
    memo.set('b', '/b');
    memo.set('c', '/c');
    expect(memo.get('a')).toBe(undefined);
    expect(memo.get('c')).toBe('/c');
    memo.delete('c');
    expect(memo.get('c')).toBe(undefined);
    expect(memo.size()).toBe(1);
  });

  it('moves a re-set url to the back of the eviction order', () => {
    const memo = createPathMemo(2);
    memo.set('a', '/a');
    memo.set('b', '/b');
    memo.set('a', '/a2');
    memo.set('c', '/c');
    expect(memo.get('a')).toBe('/a2');
    expect(memo.get('b')).toBe(undefined);
  });

  it('is bounded well past one screen of covers', () => {
    expect(IMAGE_PATH_MEMO_SIZE).toBeGreaterThanOrEqual(1024);
  });
});

describe('startImageLoad', () => {
  /** A backend call the test settles by hand. */
  function deferred() {
    const calls: { url: string; resolve: (p: string) => void; reject: (e: unknown) => void }[] = [];
    const ensure = (url: string) =>
      new Promise<string>((resolve, reject) => calls.push({ url, resolve, reject }));
    return { calls, ensure };
  }

  it('asks the backend once its slot is granted, and remembers the file', async () => {
    const limiter = createLoadLimiter(1);
    const memo = createPathMemo(8);
    const { calls, ensure } = deferred();
    const load = startImageLoad('u', ensure, limiter, memo);
    await flush();
    expect(calls.map((c) => c.url)).toEqual(['u']);
    calls[0].resolve('/cache/u.png');
    await expect(load.path).resolves.toBe('/cache/u.png');
    expect(memo.get('u')).toBe('/cache/u.png');
    // Still held: the `<img>` has not loaded yet.
    expect(limiter.active()).toBe(1);
    load.done();
    expect(limiter.active()).toBe(0);
  });

  it('holds a granted slot until the backend call settles, even once cancelled', async () => {
    const limiter = createLoadLimiter(1);
    const memo = createPathMemo(8);
    const { calls, ensure } = deferred();
    const first = startImageLoad('a', ensure, limiter, memo);
    startImageLoad('b', ensure, limiter, memo);
    await flush();
    first.cancel();
    await flush();
    // The scrolled-past card's fetch is still running, so the next one waits.
    expect(limiter.active()).toBe(1);
    expect(calls.map((c) => c.url)).toEqual(['a']);
    calls[0].resolve('/a');
    await flush();
    expect(calls.map((c) => c.url)).toEqual(['a', 'b']);
    // Its file is still remembered for when the card comes back.
    expect(memo.get('a')).toBe('/a');
  });

  it('withdraws a waiting slot at once when cancelled', async () => {
    const limiter = createLoadLimiter(1);
    const { calls, ensure } = deferred();
    startImageLoad('a', ensure, limiter, createPathMemo(8));
    const waiting = startImageLoad('b', ensure, limiter, createPathMemo(8));
    waiting.cancel();
    expect(limiter.waiting()).toBe(0);
    calls[0]?.resolve('/a');
    await flush();
    expect(calls.map((c) => c.url)).toEqual(['a']);
  });

  it('never calls the backend for a load cancelled between grant and call', async () => {
    const limiter = createLoadLimiter(1);
    const { calls, ensure } = deferred();
    const load = startImageLoad('a', ensure, limiter, createPathMemo(8));
    // Granted synchronously, but the backend call waits for a microtask.
    load.cancel();
    await flush();
    expect(calls).toHaveLength(0);
    expect(limiter.active()).toBe(0);
  });

  it('frees the slot and rejects when the backend fails', async () => {
    const limiter = createLoadLimiter(1);
    const memo = createPathMemo(8);
    const { calls, ensure } = deferred();
    const load = startImageLoad('a', ensure, limiter, memo);
    await flush();
    calls[0].reject(new Error('offline'));
    await expect(load.path).rejects.toThrow('offline');
    expect(limiter.active()).toBe(0);
    expect(memo.get('a')).toBe(undefined);
  });
});

/** The webview has `IntersectionObserver`; the node test runner does not.
 *  Same shape as the stub in visibleWarm.test.ts. */
class StubObserver {
  static instances: StubObserver[] = [];
  targets: unknown[] = [];
  callback: (entries: { target: unknown; isIntersecting: boolean }[]) => void;
  options: { root?: Element | null; rootMargin?: string } | undefined;

  constructor(
    callback: (entries: { target: unknown; isIntersecting: boolean }[]) => void,
    options?: { root?: Element | null; rootMargin?: string }
  ) {
    this.callback = callback;
    this.options = options;
    StubObserver.instances.push(this);
  }

  observe(target: unknown): void {
    if (!this.targets.includes(target)) this.targets.push(target);
  }

  unobserve(target: unknown): void {
    this.targets = this.targets.filter((t) => t !== target);
  }

  disconnect(): void {
    this.targets = [];
  }

  enter(target: unknown, isIntersecting = true): void {
    if (!this.targets.includes(target)) return;
    this.callback([{ target, isIntersecting }]);
  }
}

describe('watchNear', () => {
  const original = Reflect.get(globalThis, 'IntersectionObserver');
  // `scrollParent` walks `parentElement`; a stand-in with none has no
  // scrolling ancestor, so the root is the viewport (`null`).
  const el = () => ({ parentElement: null }) as unknown as Element;

  beforeEach(() => {
    StubObserver.instances = [];
    resetNearWatchesForTest();
    Reflect.set(globalThis, 'IntersectionObserver', StubObserver);
  });

  afterEach(() => {
    resetNearWatchesForTest();
    if (original === undefined) Reflect.deleteProperty(globalThis, 'IntersectionObserver');
    else Reflect.set(globalThis, 'IntersectionObserver', original);
  });

  it('reports every change of nearness, with the near margin', () => {
    const target = el();
    const onChange = vi.fn();
    watchNear(target, onChange);
    const observer = StubObserver.instances[0];
    expect(observer.options?.rootMargin).toBe(IMAGE_NEAR_MARGIN);

    observer.enter(target);
    observer.enter(target, false);
    observer.enter(target);
    expect(onChange.mock.calls).toEqual([[true], [false], [true]]);
  });

  it('shares one observer per scroll container', () => {
    watchNear(el(), () => {});
    watchNear(el(), () => {});
    expect(StubObserver.instances).toHaveLength(1);
    expect(StubObserver.instances[0].targets).toHaveLength(2);
  });

  it('stops watching when cancelled', () => {
    const target = el();
    const onChange = vi.fn();
    const stop = watchNear(target, onChange);
    stop();
    StubObserver.instances[0].enter(target);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('reports near at once without IntersectionObserver', () => {
    Reflect.deleteProperty(globalThis, 'IntersectionObserver');
    const onChange = vi.fn();
    watchNear(el(), onChange);
    expect(onChange.mock.calls).toEqual([[true]]);
  });
});
