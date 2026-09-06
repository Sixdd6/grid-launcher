import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { warmBackground, setScrollIdle } = vi.hoisted(() => ({
  warmBackground: vi.fn(),
  setScrollIdle: vi.fn(),
}));
// `SCROLL_IDLE_MS` is a plain number here, so the timer assertions read in
// terms of the real constant; `backgroundPrefetch.test.ts` pins its value.
vi.mock('./backgroundPrefetch', () => ({ warmBackground, setScrollIdle, SCROLL_IDLE_MS: 250 }));

import { SCROLL_IDLE_MS } from './backgroundPrefetch';
import { attachScrollGate, createVisibleWarmer, scrollParent, WARM_ROOT_MARGIN } from './visibleWarm';

/** The webview has `IntersectionObserver`; the node test runner does not.
 *  This stub records what was observed and lets a test deliver entries. */
class StubObserver {
  static instances: StubObserver[] = [];
  targets: unknown[] = [];
  disconnected = false;
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
  return { grid: { children, parentElement: null } as unknown as HTMLElement, children };
}

beforeEach(() => {
  StubObserver.instances = [];
  warmBackground.mockReset();
  warmBackground.mockReturnValue(true);
  setScrollIdle.mockReset();
  Reflect.set(globalThis, 'IntersectionObserver', StubObserver);
});

afterEach(() => {
  if (original === undefined) Reflect.deleteProperty(globalThis, 'IntersectionObserver');
  else Reflect.set(globalThis, 'IntersectionObserver', original);
});

const subject = (index: number) => ({ fanart: [], screenshots: [], cover: `c-${index}.png` });

describe('createVisibleWarmer', () => {
  it('observes every child of the grid, a row ahead of the scroll container', () => {
    const { grid, children } = fakeGrid(3);
    createVisibleWarmer(subject).observe(grid);

    expect(StubObserver.instances).toHaveLength(1);
    expect(StubObserver.instances[0].options?.rootMargin).toBe(WARM_ROOT_MARGIN);
    // No scrolling ancestor on this bare stand-in, so the viewport it is.
    expect(StubObserver.instances[0].options?.root).toBe(null);
    expect(StubObserver.instances[0].targets).toEqual(children);
  });

  it('keeps watching a card the switched-off background could not warm', () => {
    warmBackground.mockReturnValue(false);
    const { grid, children } = fakeGrid(2);
    createVisibleWarmer(subject).observe(grid);

    const observer = StubObserver.instances[0];
    observer.enter(children[0]);
    expect(warmBackground).toHaveBeenCalledOnce();
    expect(observer.targets).toContain(children[0]);
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

describe('scrollParent', () => {
  /** A minimal element chain: `parentElement` and nothing else, since that is
   *  all the walk reads. */
  function chain(depth: number): { el: Element; nodes: Element[] } {
    const nodes: Element[] = [];
    let parent: Element | null = null;
    for (let i = 0; i < depth; i += 1) {
      const node = { parentElement: parent } as unknown as Element;
      nodes.push(node);
      parent = node;
    }
    // `nodes` runs outermost -> innermost; the walk starts at the last.
    return { el: nodes[nodes.length - 1], nodes };
  }

  function withOverflow(overflow: Map<Element, string>, run: () => void): void {
    const original = Reflect.get(globalThis, 'getComputedStyle');
    Reflect.set(globalThis, 'getComputedStyle', (node: Element) => ({
      overflowY: overflow.get(node) ?? 'visible',
    }));
    try {
      run();
    } finally {
      if (original === undefined) Reflect.deleteProperty(globalThis, 'getComputedStyle');
      else Reflect.set(globalThis, 'getComputedStyle', original);
    }
  }

  it('returns the nearest ancestor that scrolls', () => {
    const { el, nodes } = chain(4);
    // nodes: [outer, scroller, plain, el]
    withOverflow(new Map([[nodes[1], 'auto']]), () => {
      expect(scrollParent(el)).toBe(nodes[1]);
    });
  });

  it('takes an explicit `scroll` as readily as `auto`', () => {
    const { el, nodes } = chain(3);
    withOverflow(new Map([[nodes[1], 'scroll']]), () => {
      expect(scrollParent(el)).toBe(nodes[1]);
    });
  });

  it('stops at the innermost scroller, not the outermost', () => {
    const { el, nodes } = chain(4);
    withOverflow(
      new Map([
        [nodes[0], 'auto'],
        [nodes[2], 'auto'],
      ]),
      () => {
        expect(scrollParent(el)).toBe(nodes[2]);
      }
    );
  });

  it('never returns the element itself, only an ancestor', () => {
    const { el, nodes } = chain(2);
    withOverflow(new Map([[el, 'auto']]), () => {
      expect(scrollParent(el)).toBe(null);
      expect(nodes).toHaveLength(2);
    });
  });

  it('falls back to the viewport when nothing between it and the document scrolls', () => {
    const { el } = chain(3);
    withOverflow(new Map(), () => {
      expect(scrollParent(el)).toBe(null);
    });
  });
});

describe('attachScrollGate', () => {
  type Recorded = { type: string; fn: () => void; options?: unknown };

  /** A stand-in scroll container that records the listeners put on it, so
   *  both the attach and the detach are observable. */
  function fakeScroller(): {
    scroller: Element;
    added: Recorded[];
    removed: Recorded[];
    scroll: () => void;
  } {
    const added: Recorded[] = [];
    const removed: Recorded[] = [];
    const scroller = {
      addEventListener: (type: string, fn: () => void, options?: unknown) => {
        added.push({ type, fn, options });
      },
      removeEventListener: (type: string, fn: () => void, options?: unknown) => {
        removed.push({ type, fn, options });
      },
    } as unknown as Element;
    return {
      scroller,
      added,
      removed,
      scroll: () => {
        for (const listener of added) if (listener.type === 'scroll') listener.fn();
      },
    };
  }

  const scrollListeners = (recorded: Recorded[]) => recorded.filter((l) => l.type === 'scroll');
  const releases = () => setScrollIdle.mock.calls.filter(([idle]) => idle === true);

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('listens for scrolls on the container it was given, passively', () => {
    const { scroller, added } = fakeScroller();
    attachScrollGate(scroller);

    expect(scrollListeners(added)).toHaveLength(1);
    // Passive: the handler never calls `preventDefault`, and a non-passive
    // scroll listener is exactly what makes a scroll wait on the main thread.
    expect(scrollListeners(added)[0].options).toEqual({ passive: true });
  });

  it('pauses the warm lane on a scroll and releases it once the grid is still', () => {
    const { scroller, scroll } = fakeScroller();
    attachScrollGate(scroller);

    scroll();
    expect(setScrollIdle).toHaveBeenCalledExactlyOnceWith(false);

    vi.advanceTimersByTime(SCROLL_IDLE_MS - 1);
    expect(releases()).toHaveLength(0);

    vi.advanceTimersByTime(1);
    expect(setScrollIdle).toHaveBeenLastCalledWith(true);
  });

  it('re-arms the trailing timer on every scroll event', () => {
    const { scroller, scroll } = fakeScroller();
    attachScrollGate(scroller);

    scroll();
    vi.advanceTimersByTime(SCROLL_IDLE_MS - 50);
    scroll();
    // The first timer would have fired here; the second scroll replaced it.
    vi.advanceTimersByTime(SCROLL_IDLE_MS - 50);
    expect(releases()).toHaveLength(0);

    vi.advanceTimersByTime(50);
    expect(releases()).toHaveLength(1);
  });

  it('stops listening and releases the gate when detached', () => {
    const { scroller, added, removed, scroll } = fakeScroller();
    const detach = attachScrollGate(scroller);
    scroll();
    detach();

    expect(scrollListeners(removed)).toHaveLength(1);
    expect(scrollListeners(removed)[0].fn).toBe(scrollListeners(added)[0].fn);
    // Unmounting mid-scroll must not leave the queue paused.
    expect(setScrollIdle).toHaveBeenLastCalledWith(true);

    // The pending timer went with it, so nothing releases the gate twice.
    vi.advanceTimersByTime(SCROLL_IDLE_MS * 2);
    expect(releases()).toHaveLength(1);
  });

  it('is independent of the warmer: observing and disconnecting never touch the gate', () => {
    // The views re-observe and disconnect on every rows change — an install
    // finishing mid-scroll — so the warmer must not be able to release a
    // pause the user is still scrolling through.
    const { grid } = fakeGrid(2);
    const warmer = createVisibleWarmer(subject);
    warmer.observe(grid);
    warmer.observe(grid);
    warmer.disconnect();

    expect(setScrollIdle).not.toHaveBeenCalled();
  });
});
