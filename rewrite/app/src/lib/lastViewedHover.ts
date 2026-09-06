// Shared dwell-timer factory for feeding `lastViewed` from a card grid's
// hover events and from keyboard/gamepad focus (design §3). Library.svelte
// and Server.svelte each mount one per input rather than duplicating the
// timer bookkeeping.
import { HOVER_DELAY_MS, PREFETCH_DELAY_MS } from './background';
import type { BackgroundSubject } from './background';
import { prefetchBackground } from './backgroundPrefetch';
import { noteViewed } from './stores/lastViewed.svelte';

/**
 * Design §3: a card becomes the background only after the pointer (or the
 * selection) has rested on it for more than `delayMs` (120ms by default).
 * Shorter dwells are pointer travel, or a held arrow key, not interest.
 */
export function createHoverViewed(
  delayMs: number = HOVER_DELAY_MS,
  prefetchMs: number = PREFETCH_DELAY_MS
): {
  start: (subject: BackgroundSubject) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let prefetchTimer: ReturnType<typeof setTimeout> | null = null;

  function clearBoth(): void {
    if (timer !== null) clearTimeout(timer);
    if (prefetchTimer !== null) clearTimeout(prefetchTimer);
    timer = null;
    prefetchTimer = null;
  }

  function start(subject: BackgroundSubject): void {
    clearBoth();
    // The fetch is the slow part (network, decode, blur), so it is not tied
    // to the visual dwell: by default it leaves NOW, in the same task as the
    // pointer event, while the swap still waits out `delayMs`. A zero timer
    // would push it behind the next macrotask for nothing, so 0 means "call
    // it", not "arm a timer". A caller that wants the fetch held back passes
    // a `prefetchMs` above 0 and gets the old two-timer shape.
    if (prefetchMs <= 0) prefetchBackground(subject);
    else
      prefetchTimer = setTimeout(() => {
        prefetchTimer = null;
        prefetchBackground(subject);
      }, prefetchMs);
    timer = setTimeout(() => {
      timer = null;
      noteViewed(subject);
    }, delayMs);
  }

  function end(): void {
    clearBoth();
  }

  return { start, end };
}
