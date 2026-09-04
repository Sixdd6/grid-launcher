// The cover the background art is showing. Module scoped so it survives a
// Shell remount, like `appUpdate.svelte.ts`.
import type { InstalledGame } from '../api';
import { startupCover } from '../background';

const state = $state<{ coverUrl: string | null; seeded: boolean }>({
  coverUrl: null,
  seeded: false,
});

export const lastViewed = {
  get coverUrl(): string | null {
    return state.coverUrl;
  },
};

/** A details popup opened, or a card was hovered past the dwell. Blank and
 *  missing covers are ignored: keeping the previous art beats a blank frame. */
export function noteViewed(coverUrl: string | null | undefined): void {
  if (typeof coverUrl !== 'string') return;
  const trimmed = coverUrl.trim();
  if (trimmed === '') return;
  state.coverUrl = trimmed;
  state.seeded = true;
}

/** The startup fallback. Runs once, and never overwrites a real view. */
export function seedLastViewed(rows: InstalledGame[]): void {
  if (state.seeded) return;
  const cover = startupCover(rows);
  if (cover === null) return;
  state.coverUrl = cover;
  state.seeded = true;
}
