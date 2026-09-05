// What the background art is showing. Module scoped so it survives a Shell
// remount, like `appUpdate.svelte.ts`.
import type { InstalledGame } from '../api';
import {
  backgroundUrls,
  EMPTY_BACKGROUND,
  isEmptySubject,
  startupSubject,
  type BackgroundSubject,
} from '../background';

const state = $state<{ subject: BackgroundSubject; seeded: boolean }>({
  subject: EMPTY_BACKGROUND,
  seeded: false,
});

export const lastViewed = {
  get subject(): BackgroundSubject {
    return state.subject;
  },
  /** The chosen tier's URLs, in cycle order. `[]` when there is no art. */
  get urls(): string[] {
    return backgroundUrls(state.subject);
  },
};

/** A details popup opened, a card was focused, or a card was hovered past the
 *  dwell. A subject with no art at all is ignored: keeping the previous art
 *  beats a blank frame. */
export function noteViewed(subject: BackgroundSubject): void {
  if (isEmptySubject(subject)) return;
  state.subject = subject;
  state.seeded = true;
}

/** The startup fallback. Runs once, and never overwrites a real view. */
export function seedLastViewed(rows: InstalledGame[]): void {
  if (state.seeded) return;
  const subject = startupSubject(rows);
  if (subject === null) return;
  state.subject = subject;
  state.seeded = true;
}
