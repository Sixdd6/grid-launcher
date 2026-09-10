// Settings › Updates (design §10): "app version, last check, release link,
// 'check-only' note". Pure; UpdatesPage.svelte reads the stores. The three
// states come from the backend's `checked_at` (Task 3): absent means the
// check was skipped or failed, and the page must not claim "up to date".
import type { AppUpdateNotice } from '../api';

export function versionLine(version: string): string {
  const v = version.trim();
  return v === '' ? 'GRID Launcher (version unknown)' : `GRID Launcher ${v}`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/**
 * How long ago the backend stamped `checked_at` (RFC 3339 UTC). Coarse on
 * purpose: the page is not a log. Empty for a stamp `Date.parse` rejects,
 * so the caller can drop the clause instead of printing "NaN min ago".
 */
export function relativeCheckTime(checkedAt: string, nowMs: number): string {
  const at = Date.parse(checkedAt);
  if (Number.isNaN(at)) return '';
  const elapsed = Math.max(0, nowMs - at);
  if (elapsed < MINUTE) return 'just now';
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)} min ago`;
  return `${Math.floor(elapsed / HOUR)} h ago`;
}

/** The notice sentence is the badge's tooltip verbatim, so the two never disagree. */
export function updateStatusLine(
  notice: AppUpdateNotice | null,
  checkedAt: string | null,
  nowMs: number,
): string {
  if (notice !== null) return `GRID Launcher ${notice.tag} is available`;
  if (checkedAt === null) return 'Not checked yet';
  const relative = relativeCheckTime(checkedAt, nowMs);
  return relative === '' ? 'Up to date' : `Up to date · checked ${relative}`;
}

/** Doc 10 D-10-h: the launcher only ever checks. */
export const CHECK_ONLY_NOTE =
  'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.';
