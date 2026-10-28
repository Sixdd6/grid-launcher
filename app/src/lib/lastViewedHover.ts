// Shared dwell-timer factory for feeding `lastViewed` from a card grid's
// hover events and from keyboard focus (design §3). Library.svelte
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
 *
 * `restartOn`: for a pointer hover, the window. Every `pointermove` on it
 * while a dwell is pending starts the dwell over, so the swap waits for the
 * pointer to STOP on a card, not merely to enter it. Counted from entry, a
 * pointer browsing across a row at a normal pace changed the art on nearly
 * every card it crossed, and each change is a full-window cross-fade: 23ms
 * frames against 16ms with the art hidden (2026-10-28). The fetch still
 * leaves on entry. Keyboard focus passes nothing: a held arrow key is
 * already one start per step.
 */
export function createHoverViewed(
  delayMs: number = HOVER_DELAY_MS,
  prefetchMs: number = PREFETCH_DELAY_MS,
  restartOn: EventTarget | null = null
): {
  start: (subject: BackgroundSubject) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let prefetchTimer: ReturnType<typeof setTimeout> | null = null;
  /** The subject whose dwell is pending, for `onMove` to re-arm. */
  let pending: BackgroundSubject | null = null;

  function arm(subject: BackgroundSubject): void {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      pending = null;
      restartOn?.removeEventListener('pointermove', onMove);
      noteViewed(subject);
    }, delayMs);
  }

  function onMove(): void {
    if (pending !== null) arm(pending);
  }

  function clearBoth(): void {
    if (timer !== null) clearTimeout(timer);
    if (prefetchTimer !== null) clearTimeout(prefetchTimer);
    timer = null;
    prefetchTimer = null;
    pending = null;
    restartOn?.removeEventListener('pointermove', onMove);
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
    pending = subject;
    // Passive and only while a dwell is pending: nothing listens while the
    // pointer rests or is away from the grid.
    restartOn?.addEventListener('pointermove', onMove, { passive: true });
    arm(subject);
  }

  function end(): void {
    clearBoth();
  }

  return { start, end };
}
