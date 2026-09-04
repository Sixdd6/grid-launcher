// Shared dwell-timer factory for feeding `lastViewed` from a card grid's
// hover events (design §3). Library.svelte and Server.svelte each mount one
// of these rather than duplicating the timer bookkeeping.
import { HOVER_DELAY_MS } from './background';
import { noteViewed } from './stores/lastViewed.svelte';

/**
 * Design §3: a card becomes the background only after the pointer has
 * rested on it for more than `delayMs` (500ms by default). Shorter dwells
 * are pointer travel, not interest.
 */
export function createHoverViewed(delayMs: number = HOVER_DELAY_MS): {
  start: (cover: string | null | undefined) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;

  function start(cover: string | null | undefined): void {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      noteViewed(cover);
    }, delayMs);
  }

  function end(): void {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  }

  return { start, end };
}
