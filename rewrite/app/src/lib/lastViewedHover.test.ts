import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { noteViewed } = vi.hoisted(() => ({ noteViewed: vi.fn() }));
vi.mock('./stores/lastViewed.svelte', () => ({ noteViewed }));

import { createHoverViewed } from './lastViewedHover';

beforeEach(() => {
  vi.useFakeTimers();
  noteViewed.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('createHoverViewed', () => {
  it('does not note anything before the dwell elapses', () => {
    const hover = createHoverViewed(500);
    hover.start('https://romm/cover.png');
    vi.advanceTimersByTime(499);
    expect(noteViewed).not.toHaveBeenCalled();
  });

  it('notes the cover once the dwell elapses', () => {
    const hover = createHoverViewed(500);
    hover.start('https://romm/cover.png');
    vi.advanceTimersByTime(500);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith('https://romm/cover.png');
  });

  it('cancels the dwell when the pointer leaves early', () => {
    const hover = createHoverViewed(500);
    hover.start('https://romm/cover.png');
    vi.advanceTimersByTime(200);
    hover.end();
    vi.advanceTimersByTime(1000);
    expect(noteViewed).not.toHaveBeenCalled();
  });

  it('a new hover restarts the dwell instead of stacking timers', () => {
    const hover = createHoverViewed(500);
    hover.start('https://romm/first.png');
    vi.advanceTimersByTime(300);
    hover.start('https://romm/second.png');
    vi.advanceTimersByTime(300);
    expect(noteViewed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(200);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith('https://romm/second.png');
  });

  it('defaults to the 500ms design dwell when no delay is given', () => {
    const hover = createHoverViewed();
    hover.start('https://romm/cover.png');
    vi.advanceTimersByTime(499);
    expect(noteViewed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith('https://romm/cover.png');
  });
});
