// Recently added's platform filter pills, the pure half. The platforms come
// from Rust (`discover_feed`'s `recent_platforms`, most recent addition
// first); a filtered list comes from `discover_recently_added`. Here: which
// pills show, what a click selects, and which list is on screen.

import type { GameSummary, RecentPlatform } from '../api';

/** The pills to show: none when fewer than two platforms are there (a
 *  filter that changes nothing is noise). */
export function filterPills(platforms: readonly RecentPlatform[]): readonly RecentPlatform[] {
  return platforms.length > 1 ? platforms : [];
}

/** The filter after a click on `clicked` (`null` is All): the pressed pill
 *  again, or All, clears it; any other pill selects that platform. */
export function nextFilter(current: number | null, clicked: number | null): number | null {
  if (clicked === null || clicked === current) return null;
  return clicked;
}

/** `current` when it is still one of the pills shown for `platforms`, else
 *  `null` (a reload dropped it, or the row is hidden). */
export function keptFilter(current: number | null, platforms: readonly RecentPlatform[]): number | null {
  if (current === null) return null;
  return filterPills(platforms).some((p) => p.id === current) ? current : null;
}

/** A filtered list on screen: the platform and its games. */
export type Filtered = { id: number; games: GameSummary[] };

/** The list Recently added shows: the filtered one once it has arrived,
 *  else the feed's own. */
export function shownList(all: GameSummary[], filtered: Filtered | null): GameSummary[] {
  return filtered ? filtered.games : all;
}

/** What restarts the rotation at the first row: the platform on screen. */
export function listKey(filtered: Filtered | null): string {
  return filtered ? `platform-${filtered.id}` : 'all';
}
