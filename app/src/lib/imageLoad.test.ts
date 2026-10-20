import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createLoadLimiter,
  IMAGE_NEAR_MARGIN,
  MAX_CONCURRENT_IMAGE_LOADS,
  resetNearWatchesForTest,
  whenNear,
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

describe('whenNear', () => {
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

  it('calls back once the element comes near, then stops watching it', () => {
    const target = el();
    const onNear = vi.fn();
    whenNear(target, onNear);
    const observer = StubObserver.instances[0];
    expect(observer.options?.rootMargin).toBe(IMAGE_NEAR_MARGIN);

    observer.enter(target, false);
    expect(onNear).not.toHaveBeenCalled();
    observer.enter(target);
    expect(onNear).toHaveBeenCalledTimes(1);
    expect(observer.targets).not.toContain(target);
  });

  it('shares one observer per scroll container', () => {
    whenNear(el(), () => {});
    whenNear(el(), () => {});
    expect(StubObserver.instances).toHaveLength(1);
    expect(StubObserver.instances[0].targets).toHaveLength(2);
  });

  it('stops watching when cancelled', () => {
    const target = el();
    const onNear = vi.fn();
    const stop = whenNear(target, onNear);
    stop();
    StubObserver.instances[0].enter(target);
    expect(onNear).not.toHaveBeenCalled();
  });

  it('calls back at once without IntersectionObserver', () => {
    Reflect.deleteProperty(globalThis, 'IntersectionObserver');
    const onNear = vi.fn();
    whenNear(el(), onNear);
    expect(onNear).toHaveBeenCalledTimes(1);
  });
});
