// Art for a game on a backup drive (Library › Backup drives, Details from a
// drive). A drive game names its covers, fanart and screenshots by paths
// relative to `gridpack/`; `drive_asset` copies one into the image cache
// and answers the cached file. The views hand `Image.svelte` a URL, so a
// drive asset travels as a `gridpack-asset:` URL that names the drive and
// the path, and `loadImagePath` sends it to `drive_asset` instead of
// `ensure_image`.
import { api } from '../api';

const SCHEME = 'gridpack-asset:';

/**
 * The URL `Image.svelte` loads a drive asset by, or `null` when the pack
 * names none (`''`). The drive key (a mount path or a drive letter) is
 * encoded, so the first `/` after it separates it from the path.
 */
export function driveAssetUrl(key: string, rel: string): string | null {
  if (rel.trim() === '') return null;
  return `${SCHEME}${encodeURIComponent(key)}/${rel}`;
}

/** The drive key and pack path a `driveAssetUrl` names, or `null` for any other URL. */
export function parseDriveAssetUrl(url: string): { key: string; rel: string } | null {
  if (!url.startsWith(SCHEME)) return null;
  const rest = url.slice(SCHEME.length);
  const slash = rest.indexOf('/');
  if (slash <= 0 || slash === rest.length - 1) return null;
  try {
    return { key: decodeURIComponent(rest.slice(0, slash)), rel: rest.slice(slash + 1) };
  } catch {
    return null;
  }
}

/** Whether `url` is a drive asset: art that exists only on a drive, which the background layer cannot blur. */
export function isDriveAssetUrl(url: string): boolean {
  return url.startsWith(SCHEME);
}

/** The cached file for an image URL: a drive asset through `drive_asset`, anything else through `ensure_image`. */
export function loadImagePath(url: string): Promise<string> {
  const asset = parseDriveAssetUrl(url);
  return asset === null ? api.ensureImage(url) : api.driveAsset(asset.key, asset.rel);
}
