import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { noteViewed } = vi.hoisted(() => ({ noteViewed: vi.fn() }));
vi.mock('./stores/lastViewed.svelte', () => ({ noteViewed }));

const { prefetchBackground } = vi.hoisted(() => ({ prefetchBackground: vi.fn() }));
vi.mock('./backgroundPrefetch', () => ({ prefetchBackground }));

import { createHoverViewed } from './lastViewedHover';

// The dwell timer only carries the subject through; the priority rule that
// picks a URL out of it lives in `background.ts`.
const subject = (cover: string) => ({ fanart: [], screenshots: [], cover });

beforeEach(() => {
  vi.useFakeTimers();
  noteViewed.mockClear();
  prefetchBackground.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('createHoverViewed', () => {
  it('does not note anything before the dwell elapses', () => {
    const hover = createHoverViewed(500);
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(499);
    expect(noteViewed).not.toHaveBeenCalled();
  });

  it('notes the cover once the dwell elapses', () => {
    const hover = createHoverViewed(500);
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(500);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
  });

  it('cancels the dwell when the pointer leaves early', () => {
    const hover = createHoverViewed(500);
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(200);
    hover.end();
    vi.advanceTimersByTime(1000);
    expect(noteViewed).not.toHaveBeenCalled();
  });

  it('a new hover restarts the dwell instead of stacking timers', () => {
    const hover = createHoverViewed(500);
    hover.start(subject('https://romm/first.png'));
    vi.advanceTimersByTime(300);
    hover.start(subject('https://romm/second.png'));
    vi.advanceTimersByTime(300);
    expect(noteViewed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(200);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/second.png'));
  });

  it('defaults to the 120ms design dwell when no delay is given', () => {
    const hover = createHoverViewed();
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(119);
    expect(noteViewed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
  });

  it('starts the fetch inside start(), before any timer runs, and swaps at 120ms', () => {
    const hover = createHoverViewed();
    hover.start(subject('https://romm/cover.png'));

    // No `advanceTimersByTime` between the two: a zero prefetch delay must
    // not cost the request a trip through the task queue.
    expect(prefetchBackground).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
    expect(noteViewed).not.toHaveBeenCalled();

    vi.advanceTimersByTime(120);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
  });

  it('fetches once per hover, not once per pending timer', () => {
    const hover = createHoverViewed();
    hover.start(subject('https://romm/first.png'));
    hover.start(subject('https://romm/second.png'));
    vi.advanceTimersByTime(1000);

    expect(prefetchBackground).toHaveBeenCalledTimes(2);
    expect(prefetchBackground).toHaveBeenLastCalledWith(subject('https://romm/second.png'));
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/second.png'));
  });

  it('still holds the fetch back when a caller asks for a delay', () => {
    const hover = createHoverViewed(500, 150);
    hover.start(subject('https://romm/cover.png'));

    vi.advanceTimersByTime(149);
    expect(prefetchBackground).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(prefetchBackground).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
    expect(noteViewed).not.toHaveBeenCalled();

    vi.advanceTimersByTime(350);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
  });

  it('cancels the prefetch too when the pointer leaves early', () => {
    const hover = createHoverViewed(500, 150);
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(100);
    hover.end();
    vi.advanceTimersByTime(1000);
    expect(prefetchBackground).not.toHaveBeenCalled();
    expect(noteViewed).not.toHaveBeenCalled();
  });
});
