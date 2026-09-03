// The Details overlay's single input contract (task-9-brief.md). It can be
// opened from either the Server grid (a `GameSummary` fetched live from the
// RomM server) or the Library grid (an `InstalledGame` registry row already
// on disk) — `DetailsSubject` normalizes both into one shape so Details.svelte
// itself never has to branch on which grid opened it.
import type { GameSummary, InstalledGame } from '../api';

export type DetailsSubject = {
  romId: number | null;
  name: string;
  platformName: string;
  coverSmall: string | null;
  coverLarge: string | null;
  screenshotUrls: string[];
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
    screenshotUrls: [],
    description: '',
    rating: '',
    genres: '',
    source: 'server',
  };
}

/**
 * `screenshot_urls` is stored as the backend's own newline-joined text
 * (install_metadata.py:111, "\n".join(screenshots)) — the backend already
 * filters out blank entries before joining, but split defensively here too:
 * trim each line and drop anything empty.
 */
export function fromInstalled(row: InstalledGame): DetailsSubject {
  return {
    romId: row.rom_id,
    name: row.title,
    platformName: row.platform,
    coverSmall: row.cover_small_path,
    coverLarge: row.cover_large_path,
    screenshotUrls: row.screenshot_urls
      .split('\n')
      .map((url) => url.trim())
      .filter((url) => url.length > 0),
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
  };
}
