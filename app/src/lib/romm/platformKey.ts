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

/**
 * Each server platform's rail slug, by platform id: its slug, or
 * `<slug>-<id>` when a platform with a lower id has that slug already (the
 * list's own order, by label, does not decide). RomM allows two platforms on
 * one slug (a custom-named "PC Fangames & Ports" on `win` beside Windows);
 * each still gets its own rail row.
 */
export function railSlugs(serverPlatforms: readonly Platform[]): Map<number, string> {
  const out = new Map<number, string>();
  const taken = new Set<string>();
  for (const p of [...serverPlatforms].sort((a, b) => a.id - b.id)) {
    const slug = serverPlatformSlug(p);
    const rail = taken.has(slug) ? `${slug}-${p.id}` : slug;
    taken.add(slug);
    out.set(p.id, rail);
  }
  return out;
}

/** Resolves installed rows to platform slugs against one server list. */
export type PlatformKeyer = (row: Pick<InstalledGame, 'platform' | 'platform_slug'>) => string;

/**
 * The slug for an installed row (plan 0.3):
 * 1. its `platform_slug` when not empty (when two server platforms share
 *    that slug, the one whose label equals the row's `platform` decides,
 *    by its rail slug, `railSlugs`);
 * 2. else the slug of the server platform whose label equals the row's
 *    `platform`;
 * 3. else the row's platform name token (`platformSlug`).
 * A token that equals a server slug joins that server platform's row: the
 * server's slug wins. Builds its label index once, so keying n rows is O(n).
 */
export function platformKeyer(serverPlatforms: readonly Platform[]): PlatformKeyer {
  const rails = railSlugs(serverPlatforms);
  const byLabel = new Map<string, string>();
  const seen = new Set<string>();
  const shared = new Set<string>();
  for (const p of serverPlatforms) {
    const slug = serverPlatformSlug(p);
    if (seen.has(slug)) shared.add(slug);
    seen.add(slug);
    const label = platformLabel(p).trim();
    if (label !== '' && !byLabel.has(label)) byLabel.set(label, rails.get(p.id) ?? slug);
  }
  return (row) => {
    const own = row.platform_slug.trim();
    if (own !== '') return shared.has(own) ? (byLabel.get(row.platform.trim()) ?? own) : own;
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
