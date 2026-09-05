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
 * selection) has rested on it for more than `delayMs` (500ms by default).
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
    // Two timers, not one: the fetch is the slow part (network, decode,
    // blur), so it starts a third of the way into the dwell, while the visual
    // swap still waits the full 500ms design §3 asks for. A dwell abandoned
    // before 150ms costs nothing at all.
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
