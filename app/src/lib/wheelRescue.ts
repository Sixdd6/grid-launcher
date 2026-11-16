// Catches wheel notches that WebKitGTK's GPU scrolling drops over a windowed
// grid, and scrolls them itself.
//
// With hardware acceleration on (WebKitGTK 2.54, Skia compositor), WebKit
// hit-tests a wheel event on its scrolling thread against a map of the
// scrolled content that the main thread records at paint time. `CardGrid`
// adds and removes cards as the list scrolls, and over a long list (about
// 200 games) that map goes stale: near the bottom of the window the wheel
// does nothing over labels, gaps and empty space, and only works again over
// a cover, whose own GPU layer keeps it in the map. Hardware acceleration
// off cures it (user report, 2026-11-16), so it is WebKit's, not the
// layout's. Chromium never shows it.
//
// The DOM still receives the event (the main thread's hit test is right), so
// a PASSIVE listener on the scroller notices the notch and opens a short
// window. Any `scroll` event in that window means the scroller took the
// notch (WebKit's smooth scroll fires them every frame), and the rescue
// stays out of the way. Only a window with no scroll at all, on a scroller
// that could have moved, is scrolled here. A frame count was not enough:
// Chromium's smooth scroll can start later than two frames, and the rescue
// then doubled a working notch.

/** How far a wheel event asks to scroll, in pixels: `deltaMode` 0 is pixels,
 *  1 lines, 2 pages. Lines use 40px, the step WebKitGTK and Chromium give
 *  one notch. */
export function wheelPixels(deltaY: number, deltaMode: number, pageHeight: number): number {
  if (deltaMode === 1) return deltaY * 40;
  if (deltaMode === 2) return deltaY * pageHeight;
  return deltaY;
}

/** Whether a scroller at `scrollTop` can still move by `dy`: not at its top
 *  when going up, not at its bottom when going down. Sub-pixel scroll
 *  positions count as at the end within one pixel. */
export function canScroll(scrollTop: number, scrollHeight: number, clientHeight: number, dy: number): boolean {
  if (dy < 0) return scrollTop > 1;
  if (dy > 0) return scrollTop + clientHeight < scrollHeight - 1;
  return false;
}

/** How long a notch may go without a `scroll` event before it counts as
 *  dropped: long enough for any smooth scroll to start, short enough that a
 *  rescued notch still feels like the wheel's. */
export const RESCUE_MS = 120;

/**
 * Watches `scroller` for dropped wheel notches and scrolls them. Returns the
 * detach. Does nothing for a pinch or Ctrl+wheel (zoom), a sideways wheel,
 * or a scroller already at the end the wheel points to.
 *
 * Notches inside one window (the first notch, then `RESCUE_MS`) are summed
 * and judged together: if the scroller sent no `scroll` event in that time,
 * the whole sum is scrolled at once, so a burst is applied exactly once.
 */
export function attachWheelRescue(scroller: Element | Window): () => void {
  // The document's own scrolling never drops a notch; only an element does.
  if (typeof window !== 'undefined' && scroller === window) return () => {};
  const el = scroller as Element;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let scrolled = false;
  let sum = 0;

  function judge(): void {
    timer = null;
    if (!scrolled) el.scrollBy({ top: sum });
  }

  function onScroll(): void {
    scrolled = true;
  }

  function onWheel(event: Event): void {
    const e = event as WheelEvent;
    if (e.ctrlKey) return;
    if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
    const dy = wheelPixels(e.deltaY, e.deltaMode, el.clientHeight);
    if (!canScroll(el.scrollTop, el.scrollHeight, el.clientHeight, dy)) return;
    if (timer !== null) {
      sum += dy;
      return;
    }
    scrolled = false;
    sum = dy;
    timer = setTimeout(judge, RESCUE_MS);
  }

  el.addEventListener('wheel', onWheel, { passive: true });
  el.addEventListener('scroll', onScroll, { passive: true });
  return () => {
    el.removeEventListener('wheel', onWheel);
    el.removeEventListener('scroll', onScroll);
    if (timer !== null) clearTimeout(timer);
  };
}
