// Shared dwell-timer factory for feeding `lastViewed` from a card grid's
// hover events and from keyboard/gamepad focus (design §3). Library.svelte
// and Server.svelte each mount one per input rather than duplicating the
// timer bookkeeping.
import { HOVER_DELAY_MS } from './background';
import type { BackgroundSubject } from './background';
import { noteViewed } from './stores/lastViewed.svelte';

/**
 * Design §3: a card becomes the background only after the pointer (or the
 * selection) has rested on it for more than `delayMs` (500ms by default).
 * Shorter dwells are pointer travel, or a held arrow key, not interest.
 */
export function createHoverViewed(delayMs: number = HOVER_DELAY_MS): {
  start: (subject: BackgroundSubject) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;

  function start(subject: BackgroundSubject): void {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      noteViewed(subject);
    }, delayMs);
  }

  function end(): void {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  }

  return { start, end };
}
