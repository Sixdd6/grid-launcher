import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createLoadLimiter,
  IMAGE_NEAR_MARGIN,
  MAX_CONCURRENT_IMAGE_LOADS,
  MAX_FAR_IMAGE_LOADS,
  resetNearWatchesForTest,
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
