import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { warmBackground } = vi.hoisted(() => ({ warmBackground: vi.fn() }));
vi.mock('./backgroundPrefetch', () => ({ warmBackground }));

import { createVisibleWarmer, WARM_ROOT_MARGIN } from './visibleWarm';

/** The webview has `IntersectionObserver`; the node test runner does not.
 *  This stub records what was observed and lets a test deliver entries. */
class StubObserver {
  static instances: StubObserver[] = [];
  targets: unknown[] = [];
  disconnected = false;
  callback: (entries: { target: unknown; isIntersecting: boolean }[]) => void;
  options: { rootMargin?: string } | undefined;

  constructor(
    callback: (entries: { target: unknown; isIntersecting: boolean }[]) => void,
    options?: { rootMargin?: string }
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
    this.disconnected = true;
    this.targets = [];
  }

  /** Delivers `target` as intersecting, the way the browser would — and
   *  only while it is still observed, which is what makes `unobserve`
   *  observable from a test. */
  enter(target: unknown): void {
    if (!this.targets.includes(target)) return;
    this.callback([{ target, isIntersecting: true }]);
  }
}

const original = Reflect.get(globalThis, 'IntersectionObserver');

/** Three childless stand-ins for cards: the warmer only ever compares them
 *  by identity against `grid.children`, so no DOM is needed. */
function fakeGrid(count: number): { grid: HTMLElement; children: unknown[] } {
  const children = Array.from({ length: count }, (_, i) => ({ card: i }));
  return { grid: { children } as unknown as HTMLElement, children };
}

beforeEach(() => {
  StubObserver.instances = [];
  warmBackground.mockClear();
  Reflect.set(globalThis, 'IntersectionObserver', StubObserver);
});

afterEach(() => {
  if (original === undefined) Reflect.deleteProperty(globalThis, 'IntersectionObserver');
  else Reflect.set(globalThis, 'IntersectionObserver', original);
});

const subject = (index: number) => ({ fanart: [], screenshots: [], cover: `c-${index}.png` });

describe('createVisibleWarmer', () => {
  it('observes every child of the grid, a row ahead', () => {
    const { grid, children } = fakeGrid(3);
    createVisibleWarmer(subject).observe(grid);

    expect(StubObserver.instances).toHaveLength(1);
    expect(StubObserver.instances[0].options?.rootMargin).toBe(WARM_ROOT_MARGIN);
    expect(StubObserver.instances[0].targets).toEqual(children);
  });

  it('warms the subject at the entry’s own index', () => {
    const { grid, children } = fakeGrid(3);
    createVisibleWarmer(subject).observe(grid);

    StubObserver.instances[0].enter(children[2]);
    expect(warmBackground).toHaveBeenCalledExactlyOnceWith(subject(2));
  });

  it('warms each card once, then stops watching it', () => {
    const { grid, children } = fakeGrid(3);
    createVisibleWarmer(subject).observe(grid);

    const observer = StubObserver.instances[0];
    observer.enter(children[1]);
    expect(observer.targets).not.toContain(children[1]);

    // Scrolling it back in cannot warm it a second time: it is no longer
    // watched, so the browser has nothing to report.
    observer.enter(children[1]);
    expect(warmBackground).toHaveBeenCalledOnce();
  });

  it('ignores an entry that is only leaving the viewport', () => {
    const { grid, children } = fakeGrid(2);
    createVisibleWarmer(subject).observe(grid);

    StubObserver.instances[0].callback([{ target: children[0], isIntersecting: false }]);
    expect(warmBackground).not.toHaveBeenCalled();
  });

  it('warms nothing for an index with no subject', () => {
    const { grid, children } = fakeGrid(2);
    createVisibleWarmer(() => null).observe(grid);

    StubObserver.instances[0].enter(children[0]);
    expect(warmBackground).not.toHaveBeenCalled();
  });

  it('re-observes the children a refresh added', () => {
    const { grid, children } = fakeGrid(2);
    const warmer = createVisibleWarmer(subject);
    warmer.observe(grid);

    const first = [...children];
    const extra = { card: 2 };
    (grid.children as unknown as unknown[]).push(extra);
    warmer.observe(grid);

    expect(StubObserver.instances).toHaveLength(1);
    expect(StubObserver.instances[0].targets).toEqual([...first, extra]);
  });

  it('disconnects the observer when the view goes away', () => {
    const { grid } = fakeGrid(2);
    const warmer = createVisibleWarmer(subject);
    warmer.observe(grid);
    warmer.disconnect();

    expect(StubObserver.instances[0].disconnected).toBe(true);
  });

  it('is a no-op where IntersectionObserver does not exist', () => {
    Reflect.deleteProperty(globalThis, 'IntersectionObserver');
    const { grid } = fakeGrid(2);
    const warmer = createVisibleWarmer(subject);

    expect(() => {
      warmer.observe(grid);
      warmer.disconnect();
    }).not.toThrow();
    expect(warmBackground).not.toHaveBeenCalled();
  });
});
