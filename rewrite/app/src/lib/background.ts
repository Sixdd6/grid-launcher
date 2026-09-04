// Pure selection logic for the shell's background art (design §3).
import type { InstalledGame } from './api';

/** Design §3: a card must be hovered for MORE than half a second before it
 *  becomes the background. Shorter dwells are pointer travel, not interest. */
export const HOVER_DELAY_MS = 500;

/** Design §3's cross-fade duration, matching the `--m-slow` CSS token
 *  `BackgroundArt.svelte` transitions opacity with. Used on the JS side to
 *  time when the outgoing layer's image is safe to drop (see
 *  `backgroundSlots.ts`) — kept as a named constant, not a re-parsed CSS
 *  value, since the two must already agree for the fade to look right. */
export const CROSS_FADE_MS = 360;

/**
 * The cover the shell starts with, before the user has viewed anything.
 *
 * The design asks for "the most recently played installed game". The
 * registry records no play timestamp — nothing in grid-core stores a
 * `last_played` — so the newest `installed_at` stands in for it: the game
 * a user just added is the one they are about to play. Revisit this when a
 * play-time column exists.
 *
 * Rows without a large cover are skipped rather than returned blank: the
 * caller would otherwise render an empty layer over a perfectly good
 * candidate further down the list.
 */
export function startupCover(rows: InstalledGame[]): string | null {
  let best: InstalledGame | null = null;
  for (const row of rows) {
    if (row.cover_large_path.trim() === '') continue;
    if (best === null || row.installed_at > best.installed_at) best = row;
  }
  return best?.cover_large_path ?? null;
}
