// Pure selection logic for the shell's background art (design §3).
import type { GameSummary, InstalledGame } from './api';
import type { DetailsSubject } from './details/subject';

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
 * How long a card must be dwelled on before its art is fetched — 150ms,
 * well under `HOVER_DELAY_MS`. The swap still happens at 500ms; this only
 * starts the (potentially slow: network + decode + blur) variant build early,
 * so the image is usually ready by the time the swap is allowed.
 */
export const PREFETCH_DELAY_MS = 150;

/**
 * How long each background image is held before the next one
 * (`FanartBackground`'s own 5000ms timer,
 * `grid_launcher/tv/widgets/components/fanart_background.py:52-53`).
 */
export const BACKGROUND_CYCLE_MS = 5000;

/**
 * Everything the background art may show for ONE game, in priority order.
 * User ruling 2026-09-05: fanart wins, then the game's own screenshots
 * (cycling), and the cover only as a last resort — a portrait cover stretched
 * across a landscape window is the worst of the three, not the first choice.
 */
export type BackgroundSubject = {
  /** Resolved + host-filtered fanart URLs; usually empty. */
  fanart: string[];
  /** Resolved + host-filtered screenshot URLs, in source order. */
  screenshots: string[];
  /** The large cover, or `null`. */
  cover: string | null;
};

export const EMPTY_BACKGROUND: BackgroundSubject = { fanart: [], screenshots: [], cover: null };

function clean(urls: readonly (string | null | undefined)[]): string[] {
  const out: string[] = [];
  for (const url of urls) {
    if (typeof url !== 'string') continue;
    const trimmed = url.trim();
    if (trimmed === '' || out.includes(trimmed)) continue;
    out.push(trimmed);
  }
  return out;
}

/** Shared, never mutated: the default `failed` set for every caller that has
 *  no failures to report. A module-level constant so the common case costs
 *  one allocation for the whole module instead of one per call, and so the
 *  default is a single object typed `ReadonlySet` that no caller can add to
 *  behind another caller's back. */
const EMPTY_SET: ReadonlySet<string> = new Set<string>();

/**
 * The URLs to show for `subject`, in order: the FIRST non-empty tier wins.
 *
 * `failed` holds URLs whose blurred variant the backend could not build. They
 * are removed from each tier BEFORE the first-non-empty rule is applied, so a
 * fanart that cannot be decoded falls through to the screenshots and then to
 * the cover instead of leaving the shell blank — on the live server more than
 * half the games have fanart, so this is the common path, not an edge case.
 */
export function backgroundUrls(
  subject: BackgroundSubject,
  failed: ReadonlySet<string> = EMPTY_SET
): string[] {
  const usable = (urls: readonly (string | null | undefined)[]) =>
    clean(urls).filter((url) => !failed.has(url));
  const fanart = usable(subject.fanart);
  if (fanart.length > 0) return fanart;
  const screenshots = usable(subject.screenshots);
  if (screenshots.length > 0) return screenshots;
  return usable([subject.cover]);
}

export function isEmptySubject(subject: BackgroundSubject): boolean {
  return backgroundUrls(subject).length === 0;
}

/** Cycle only with something to cycle to, and only while the art is visible. */
export function shouldCycle(urls: string[], fade: number): boolean {
  return urls.length > 1 && fade > 0;
}

/**
 * The next index, wrapping. `0` for an empty list — never `NaN`, and never
 * negative: JS's `%` keeps the sign of its left operand, so a caller holding a
 * negative index (a reset that raced a shrinking list) would otherwise get a
 * negative index back and read past the start of the array.
 */
export function cycleIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (((current + 1) % count) + count) % count;
}

/** The registry stores these columns as newline-joined text. */
function splitStored(stored: string | null | undefined): string[] {
  return clean((stored ?? '').split('\n'));
}

export function subjectFromInstalled(row: InstalledGame): BackgroundSubject {
  return {
    fanart: splitStored(row.fanart_urls),
    screenshots: splitStored(row.screenshot_urls),
    cover: clean([row.cover_large_path])[0] ?? null,
  };
}

export function subjectFromSummary(game: GameSummary): BackgroundSubject {
  return {
    fanart: clean(game.fanart_urls),
    screenshots: clean(game.screenshot_urls),
    cover: clean([game.path_cover_large])[0] ?? null,
  };
}

/** The merged detail the popup is showing — the richest subject there is. */
export function subjectFromDetails(subject: DetailsSubject): BackgroundSubject {
  return {
    fanart: clean(subject.fanartUrls),
    screenshots: clean(subject.screenshotUrls),
    cover: clean([subject.coverLarge])[0] ?? null,
  };
}

/**
 * The subject the shell starts with, before the user has viewed anything.
 *
 * The design asks for "the most recently played installed game". The registry
 * records no play timestamp, so the newest `installed_at` stands in for it:
 * the game a user just added is the one they are about to play. Revisit this
 * when a play-time column exists.
 *
 * Rows with no art at all are skipped rather than returned blank: the caller
 * would otherwise render an empty layer over a perfectly good candidate
 * further down the list.
 */
export function startupSubject(rows: InstalledGame[]): BackgroundSubject | null {
  let best: { row: InstalledGame; subject: BackgroundSubject } | null = null;
  for (const row of rows) {
    const subject = subjectFromInstalled(row);
    if (isEmptySubject(subject)) continue;
    if (best === null || row.installed_at > best.row.installed_at) best = { row, subject };
  }
  return best === null ? null : best.subject;
}
