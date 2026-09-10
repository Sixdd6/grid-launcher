// What the background art is showing. Module scoped so it survives a Shell
// remount, like `appUpdate.svelte.ts`.
import type { InstalledGame } from '../api';
import { backgroundUrls, EMPTY_BACKGROUND, startupSubject, type BackgroundSubject } from '../background';

// A copy, not `EMPTY_BACKGROUND` itself: `$state` deep-proxies what it is
// given, and proxying the shared constant would let a write here leak into
// every other module that imports it.
const state = $state<{ subject: BackgroundSubject; seeded: boolean }>({
  subject: { ...EMPTY_BACKGROUND },
  seeded: false,
});

export const lastViewed = {
  get subject(): BackgroundSubject {
    return state.subject;
  },
};

/**
 * A details popup opened, a card was focused, or a card was hovered past the
 * dwell. A subject with no art at all is ignored: keeping the previous art
 * beats a blank frame.
 *
 * A re-report of the SAME art keeps the current subject object. The details
 * overlay re-reports its merged subject on every field change, and
 * `BackgroundArt` resets its cycle index whenever the subject's identity
 * changes — without this gate, a rating arriving mid-cycle would snap the art
 * back to the first screenshot.
 */
export function noteViewed(subject: BackgroundSubject): void {
  const urls = backgroundUrls(subject);
  if (urls.length === 0) return;
  state.seeded = true;
  if (urls.join('\n') === backgroundUrls(state.subject).join('\n')) return;
  state.subject = subject;
}

/** The startup fallback. Runs once, and never overwrites a real view. */
export function seedLastViewed(rows: InstalledGame[]): void {
  if (state.seeded) return;
  const subject = startupSubject(rows);
  if (subject === null) return;
  state.subject = subject;
  state.seeded = true;
}
