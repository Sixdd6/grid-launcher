// The `<slug>` of a RomM platform row (`platform:<slug>`, plan 0.3), for a
// server platform and for an installed RomM row, so an unlinked or pre-v8
// row lands in its platform's row. Pure; desktop and TV share it.
import type { InstalledGame, Platform } from '../api';
import { isHiddenLibraryPlatform } from '../library';
import { platformSlug } from '../library/rail';
import { platformLabel } from '../server/platformLabel';

export type PlatformRailKey = `platform:${string}`;

/** `platform:<slug>`. */
export function platformRailKey(slug: string): PlatformRailKey {
  return `platform:${slug}`;
}

/** A server platform's slug; the label's name token when the server sent none. */
export function serverPlatformSlug(p: Pick<Platform, 'slug' | 'display_name' | 'name'>): string {
  return p.slug.trim() !== '' ? p.slug.trim() : platformSlug(platformLabel(p));
}

/** Whether a server platform is RomM's Emulators shelf (never a rail row). */
export function isEmulatorsShelf(p: Pick<Platform, 'slug' | 'display_name' | 'name'>): boolean {
  return isHiddenLibraryPlatform(platformLabel(p)) || isHiddenLibraryPlatform(p.slug);
}

/** Resolves installed rows to platform slugs against one server list. */
export type PlatformKeyer = (row: Pick<InstalledGame, 'platform' | 'platform_slug'>) => string;

/**
 * The slug for an installed row (plan 0.3):
 * 1. its `platform_slug` when not empty;
 * 2. else the slug of the server platform whose label equals the row's
 *    `platform`;
 * 3. else the row's platform name token (`platformSlug`).
 * A token that equals a server slug joins that server platform's row: the
 * server's slug wins. Builds its label index once, so keying n rows is O(n).
 */
export function platformKeyer(serverPlatforms: readonly Platform[]): PlatformKeyer {
  const byLabel = new Map<string, string>();
  for (const p of serverPlatforms) {
    const label = platformLabel(p).trim();
    if (label !== '' && !byLabel.has(label)) byLabel.set(label, serverPlatformSlug(p));
  }
  return (row) => {
    const own = row.platform_slug.trim();
    if (own !== '') return own;
    return byLabel.get(row.platform.trim()) ?? platformSlug(row.platform);
  };
}

/** One row's slug without a prepared keyer (tests, one-offs). */
export function installedPlatformSlug(
  row: Pick<InstalledGame, 'platform' | 'platform_slug'>,
  serverPlatforms: readonly Platform[],
): string {
  return platformKeyer(serverPlatforms)(row);
}
