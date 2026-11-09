// The Details overlay's single input contract (task-9-brief.md). It can be
// opened from the Server grid (a `GameSummary` fetched live from the RomM
// server), the Library grid (an `InstalledGame` registry row already on
// disk) or a backup drive's grid (a `DriveGame` from the drive's pack) —
// `DetailsSubject` normalizes all three into one shape.
import type { DriveGame, DriveInfo, GameSummary, InstalledGame, RomDetail } from '../api';
import { driveAssetUrl } from '../backups/driveArt';
import { serverKey } from '../backups/drives';

/** The drive a drive subject was opened from (GameBackups README, "Details from a drive"). */
export type SubjectDrive = {
  /** The drive key: its mount path, or its drive letter on Windows. */
  key: string;
  /** The pack name: "From Couch kit". */
  name: string;
  /** The volume label. */
  label: string;
  /** The server the game was backed up from (`DriveGame.server`). */
  server: string;
};

export type DetailsSubject = {
  romId: number | null;
  name: string;
  platformName: string;
  /**
   * The server platform's slug; nativeness reads it first (user bug
   * 2026-10-10). '' when unknown: a registry row the backfill has not
   * reached. `mergeDetail` fills it from the ROM detail.
   */
  platformSlug: string;
  coverSmall: string | null;
  coverLarge: string | null;
  screenshotUrls: string[];
  fanartUrls: string[];
  description: string;
  rating: string;
  genres: string;
  source: 'server' | 'installed' | 'drive';
  /** Set only for `source: 'drive'`. */
  drive?: SubjectDrive;
  /** The release date and companies a drive's pack carries, for the header
   *  line before (or without) a server detail. */
  releaseDate?: string;
  companies?: string;
};

export function fromSummary(game: GameSummary, platformName: string, platformSlug: string): DetailsSubject {
  return {
    romId: game.id,
    name: game.name,
    platformName,
    platformSlug,
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
    platformSlug: row.platform_slug,
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
 * A game on a backup drive. Its art is the drive's own copy, loaded through
 * `drive_asset` (`backups/driveArt.ts`), so it shows with no network.
 */
export function fromDriveGame(game: DriveGame, drive: Pick<DriveInfo, 'key' | 'label' | 'mount' | 'pack'>): DetailsSubject {
  const asset = (rel: string) => driveAssetUrl(drive.key, rel);
  const cover = asset(game.cover);
  const urls = (rels: readonly string[]) => rels.flatMap((rel) => asset(rel) ?? []);
  return {
    romId: game.rom_id,
    name: game.name,
    platformName: game.platform_name,
    platformSlug: game.platform,
    coverSmall: cover,
    coverLarge: cover,
    screenshotUrls: urls(game.screenshots),
    fanartUrls: urls([game.fanart]),
    description: game.summary,
    rating: game.rating,
    genres: game.genres.join(', '),
    source: 'drive',
    drive: {
      key: drive.key,
      name: drive.pack?.name ?? (drive.label.trim() || drive.mount),
      label: drive.label.trim() || drive.mount,
      server: game.server,
    },
    releaseDate: releaseSeconds(game.release_date),
    companies: game.companies.join(', '),
  };
}

/**
 * A pack's release date ("1993-11-11") as the epoch-seconds string the
 * header reads from a server detail (`releaseYear`), mirroring grid-core's
 * `release_date_seconds`. Seconds pass through; anything else is `''`.
 */
export function releaseSeconds(raw: string): string {
  const t = raw.trim();
  if (/^\d+$/.test(t)) return t;
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(t);
  if (m === null) return '';
  return String(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])) / 1000);
}

/** "From Couch kit", the drive part of the header line; `''` for any other subject. */
export function fromDriveText(subject: DetailsSubject): string {
  return subject.drive ? `From ${subject.drive.name}` : '';
}

/**
 * Whether the server's detail for `subject.romId` describes this game. A
 * drive game's rom id belongs to the server it was backed up from: from any
 * other server the same id is a different game, so its detail must not be
 * merged over the drive's.
 */
export function detailApplies(subject: DetailsSubject, serverUrl: string): boolean {
  if (subject.drive === undefined) return true;
  return serverUrl.trim() !== '' && serverKey(subject.drive.server) === serverKey(serverUrl);
}

/** The version-label source order (`details/version.ts`): a drive game reads like a server one. */
export function versionSource(subject: DetailsSubject): 'server' | 'installed' {
  return subject.source === 'installed' ? 'installed' : 'server';
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
    // Server-grid fields neither call site reads: their "unknown" values.
    platform_display_name: '',
    genres: [],
    created_at: '',
    first_release_date: null,
    logo_url: '',
    fs_size_bytes: 0,
    largest_file_bytes: 0,
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
    platformSlug: detail.platform_slug || subject.platformSlug,
  };
}
