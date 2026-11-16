import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { RESCUE_MS, attachWheelRescue, canScroll, wheelPixels } from './wheelRescue';

describe('wheelPixels', () => {
  it('reads pixels, lines and pages', () => {
    expect(wheelPixels(53, 0, 700)).toBe(53);
    expect(wheelPixels(3, 1, 700)).toBe(120);
    expect(wheelPixels(-1, 2, 700)).toBe(-700);
  });
});

describe('canScroll', () => {
  it('can go down until the bottom and up until the top', () => {
    expect(canScroll(0, 2000, 700, 40)).toBe(true);
    expect(canScroll(0, 2000, 700, -40)).toBe(false);
    expect(canScroll(1300, 2000, 700, 40)).toBe(false);
    expect(canScroll(1299.5, 2000, 700, 40)).toBe(false);
    expect(canScroll(1300, 2000, 700, -40)).toBe(true);
    expect(canScroll(500, 2000, 700, 0)).toBe(false);
  });
});

/** A scroller stand-in: `scrollBy` moves it, `webkitTakes` decides whether
 *  "WebKit" scrolls a notch itself (and fires `scroll`). */
function scroller(webkitTakes: boolean) {
  const listeners: Record<string, ((e: Event) => void) | undefined> = {};
  const el = {
    scrollTop: 1000,
    scrollHeight: 9000,
    clientHeight: 700,
    scrollBy: vi.fn(({ top }: { top: number }) => {
      el.scrollTop += top;
      listeners.scroll?.({} as Event);
    }),
    addEventListener: (type: string, fn: (e: Event) => void) => (listeners[type] = fn),
    removeEventListener: (type: string) => (listeners[type] = undefined),
  };
  const wheel = (deltaY: number) => {
    listeners.wheel?.({ deltaY, deltaX: 0, deltaMode: 0, ctrlKey: false } as unknown as Event);
    if (webkitTakes) {
      el.scrollTop += deltaY;
      listeners.scroll?.({} as Event);
    }
  };
  return { el, wheel, attached: () => listeners.wheel !== undefined && listeners.scroll !== undefined };
}

describe('attachWheelRescue', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('stays out of the way when WebKit scrolls the notch', () => {
    const s = scroller(true);
    attachWheelRescue(s.el as unknown as Element);
    s.wheel(53);
    vi.advanceTimersByTime(RESCUE_MS);
    expect(s.el.scrollBy).not.toHaveBeenCalled();
    expect(s.el.scrollTop).toBe(1053);
  });

  it('scrolls a dropped notch itself once the window closes', () => {
    const s = scroller(false);
    attachWheelRescue(s.el as unknown as Element);
    s.wheel(53);
    vi.advanceTimersByTime(RESCUE_MS - 1);
    expect(s.el.scrollBy).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(s.el.scrollBy).toHaveBeenCalledWith({ top: 53 });
    expect(s.el.scrollTop).toBe(1053);
  });

  it('applies a burst of dropped notches exactly once', () => {
    const s = scroller(false);
    attachWheelRescue(s.el as unknown as Element);
    s.wheel(53);
    s.wheel(53);
    vi.advanceTimersByTime(50);
    s.wheel(-20);
    vi.advanceTimersByTime(RESCUE_MS);
    expect(s.el.scrollBy).toHaveBeenCalledTimes(1);
    expect(s.el.scrollTop).toBe(1086);
    // The next notch opens a new window.
    s.wheel(53);
    vi.advanceTimersByTime(RESCUE_MS);
    expect(s.el.scrollBy).toHaveBeenCalledTimes(2);
    expect(s.el.scrollTop).toBe(1139);
  });

  it('ignores a notch at the end it points to', () => {
    const s = scroller(false);
    s.el.scrollTop = 8300;
    attachWheelRescue(s.el as unknown as Element);
    s.wheel(53);
    vi.advanceTimersByTime(RESCUE_MS);
    expect(s.el.scrollBy).not.toHaveBeenCalled();
  });

  it('detaches', () => {
    const s = scroller(false);
    const detach = attachWheelRescue(s.el as unknown as Element);
    expect(s.attached()).toBe(true);
    detach();
    expect(s.attached()).toBe(false);
  });
});
