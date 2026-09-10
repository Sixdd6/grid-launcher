// The Details overlay's single input contract (task-9-brief.md). It can be
// opened from either the Server grid (a `GameSummary` fetched live from the
// RomM server) or the Library grid (an `InstalledGame` registry row already
// on disk) — `DetailsSubject` normalizes both into one shape so Details.svelte
// itself never has to branch on which grid opened it.
import type { GameSummary, InstalledGame, RomDetail } from '../api';

export type DetailsSubject = {
  romId: number | null;
  name: string;
  platformName: string;
  coverSmall: string | null;
  coverLarge: string | null;
  screenshotUrls: string[];
  fanartUrls: string[];
  description: string;
  rating: string;
  genres: string;
  source: 'server' | 'installed';
};

export function fromSummary(game: GameSummary, platformName: string): DetailsSubject {
  return {
    romId: game.id,
    name: game.name,
    platformName,
    coverSmall: game.path_cover_small,
    coverLarge: game.path_cover_large,
    screenshotUrls: game.screenshot_urls,
    fanartUrls: game.fanart_urls,
    description: '',
    rating: '',
    genres: '',
    source: 'server',
  };
}

/** The registry stores these columns as newline-joined text; blanks are
 *  dropped defensively even though the backend already filters them. */
function splitStored(stored: string): string[] {
  return stored
    .split('\n')
    .map((url) => url.trim())
    .filter((url) => url.length > 0);
}

/**
 * `screenshot_urls` and `fanart_urls` are stored as the backend's own
 * newline-joined text (install_metadata.py:111, "\n".join(screenshots)) — see
 * `splitStored`.
 */
export function fromInstalled(row: InstalledGame): DetailsSubject {
  return {
    romId: row.rom_id,
    name: row.title,
    platformName: row.platform,
    coverSmall: row.cover_small_path,
    coverLarge: row.cover_large_path,
    screenshotUrls: splitStored(row.screenshot_urls),
    fanartUrls: splitStored(row.fanart_urls),
    description: row.description,
    rating: row.rating,
    genres: row.genres,
    source: 'installed',
  };
}

/**
 * Shim for the code Details.svelte still shares with the rest of the app in
 * `GameSummary` terms (`isInstalled`, `syntheticCloudGame`). `platform_id: 0`
 * is a harmless placeholder — both call sites take the platform name
 * separately and never read `platform_id` off the summary they're given.
 */
export function summaryOf(subject: DetailsSubject): GameSummary {
  return {
    id: subject.romId ?? 0,
    name: subject.name,
    platform_id: 0,
    path_cover_small: subject.coverSmall,
    path_cover_large: subject.coverLarge,
    screenshot_urls: subject.screenshotUrls,
    fanart_urls: subject.fanartUrls,
  };
}

/**
 * Folds a freshly fetched `RomDetail` over the subject the grid opened the
 * overlay with. `RomDetail`'s string fields are never null — the backend
 * sends `""` for "the server has nothing here" — so a naive
 * `detail.cover_large_path ?? subject.coverLarge` would replace a perfectly
 * good stored cover with an empty string and blank the cover box. Every
 * field here therefore treats `""` as absent and keeps the subject's own
 * value; `screenshot_urls` does the same with an empty list. Covers
 * normalize `""` to `null`, the shape the rest of `DetailsSubject` uses.
 */
export function mergeDetail(subject: DetailsSubject, detail: RomDetail): DetailsSubject {
  return {
    ...subject,
    coverSmall: detail.cover_small_path || subject.coverSmall || null,
    coverLarge: detail.cover_large_path || subject.coverLarge || null,
    screenshotUrls: detail.screenshot_urls.length > 0 ? detail.screenshot_urls : subject.screenshotUrls,
    fanartUrls: detail.fanart_urls.length > 0 ? detail.fanart_urls : subject.fanartUrls,
    description: detail.description || subject.description,
    rating: detail.rating || subject.rating,
    genres: detail.genres || subject.genres,
  };
}
