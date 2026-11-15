// The Library rail's RomM group (plan 0.3, decision 11, RommStorefront
// README "Library rail"). Pure; the desktop and TV rails render it.
import type { InstalledGame, Platform, RommStatus } from '../api';
import { isHiddenLibraryPlatform } from '../library';
import { platformLabel } from '../server/platformLabel';
import { isEmulatorsShelf, platformKeyer, platformRailKey, serverPlatformSlug, type PlatformRailKey } from './platformKey';
import { ROMM_FLAG_TITLES, ROMM_HEADING } from './words';

export type RommRailKey = 'romm:installed' | 'romm:discover' | PlatformRailKey;

/** The heading's badge: a spinner while connecting, `!` for Can't reach and
 *  Sign-in expired, `off` for not connected. */
export type RommFlag = { text: '!' | 'off' | 'busy'; title: string } | null;

export type RommRailRow = {
  key: RommRailKey;
  kind: 'installed' | 'discover' | 'platform';
  label: string;
  /** Installed: the installed RomM games. Platform: RomM's game count only
   *  (decision 14; 0 for a platform only installed games have). Discover:
   *  `null` (no count). */
  count: number | null;
  /** `library-rail-romm-installed`, `library-rail-romm-discover`, `library-rail-platform-<slug>`. */
  testId: string;
  /** `library-rail-count-romm-installed`, `library-rail-count-platform-<slug>`; `null` without a count. */
  countTestId: string | null;
  /** A platform row's slug; `null` otherwise. */
  slug: string | null;
  /** A platform row's server platform (for its id); `null` for an installed-only platform. */
  platform: Platform | null;
};

export type RommRailGroup = { heading: typeof ROMM_HEADING; flag: RommFlag; rows: RommRailRow[] };

/** The installed rows that belong to RomM: source RomM, not the hidden Emulators shelf. */
export function rommInstalledRows(rows: readonly InstalledGame[]): InstalledGame[] {
  return rows.filter((r) => r.source === 'romm' && !isHiddenLibraryPlatform(r.platform));
}

/**
 * Whether RomM is starting up: a probe runs for a stored connection while
 * RomM is not connected yet (the startup restore; also a connect over a
 * stored server). The rail treats it as live: Installed plus Discover.
 */
export function rommConnecting(status: Pick<RommStatus, 'connection' | 'busy' | 'server_url'>): boolean {
  return status.connection === 'not_connected' && status.busy && status.server_url !== '';
}

/** Connected, or connecting at startup: Discover (and Automatic's landing) exist. */
export function rommLive(status: Pick<RommStatus, 'connection' | 'busy' | 'server_url'>): boolean {
  return status.connection === 'connected' || rommConnecting(status);
}

export function rommFlag(status: RommStatus): RommFlag {
  if (status.connection === 'connected') return null;
  if (rommConnecting(status)) return { text: 'busy', title: ROMM_FLAG_TITLES.connecting };
  if (status.connection === 'unreachable') return { text: '!', title: ROMM_FLAG_TITLES.unreachable };
  if (status.connection === 'expired') return { text: '!', title: ROMM_FLAG_TITLES.expired };
  return { text: 'off', title: ROMM_FLAG_TITLES.off };
}

function byLabel(a: { label: string }, b: { label: string }): number {
  return a.label.localeCompare(b.label, undefined, { sensitivity: 'base' });
}

/**
 * The platform rows while connected: the union of the server's platforms
 * (with games, not the Emulators shelf) and the installed RomM platforms,
 * by label. The label is the server's when known, else the row's platform.
 */
export function rommPlatformRows(platforms: readonly Platform[], installedRows: readonly InstalledGame[]): RommRailRow[] {
  const bySlug = new Map<string, RommRailRow>();
  for (const p of platforms) {
    if (p.rom_count <= 0 || isEmulatorsShelf(p)) continue;
    const slug = serverPlatformSlug(p);
    if (bySlug.has(slug)) continue;
    bySlug.set(slug, platformRow(slug, platformLabel(p), p.rom_count, p));
  }
  const keyOf = platformKeyer(platforms);
  for (const row of rommInstalledRows(installedRows)) {
    const slug = keyOf(row);
    if (slug === '' || bySlug.has(slug)) continue;
    bySlug.set(slug, platformRow(slug, row.platform, 0, null));
  }
  return [...bySlug.values()].sort(byLabel);
}

function platformRow(slug: string, label: string, count: number, platform: Platform | null): RommRailRow {
  return {
    key: platformRailKey(slug),
    kind: 'platform',
    label,
    count,
    testId: `library-rail-platform-${slug}`,
    countTestId: `library-rail-count-platform-${slug}`,
    slug,
    platform,
  };
}

/**
 * The RomM group, or `null` when it does not show (not connected and no
 * installed RomM games). `platforms`: the server's list while connected
 * (`null` while it loads). The installed count is read from `installedRows`
 * (live), not from `status.installed_count` (a snapshot).
 *
 * - connected: Installed, Discover, then the platform rows;
 * - connecting at startup: Installed, Discover;
 * - unreachable, expired, not connected: Installed only.
 */
export function rommRailGroup(
  status: RommStatus,
  platforms: readonly Platform[] | null,
  installedRows: readonly InstalledGame[],
): RommRailGroup | null {
  const installed = rommInstalledRows(installedRows);
  const live = rommLive(status);
  if (status.connection === 'not_connected' && !live && installed.length === 0) return null;
  const rows: RommRailRow[] = [
    {
      key: 'romm:installed',
      kind: 'installed',
      label: 'Installed',
      count: installed.length,
      testId: 'library-rail-romm-installed',
      countTestId: 'library-rail-count-romm-installed',
      slug: null,
      platform: null,
    },
  ];
  if (live) {
    rows.push({
      key: 'romm:discover',
      kind: 'discover',
      label: 'Discover',
      count: null,
      testId: 'library-rail-romm-discover',
      countTestId: null,
      slug: null,
      platform: null,
    });
  }
  if (status.connection === 'connected' && platforms !== null) rows.push(...rommPlatformRows(platforms, installed));
  return { heading: ROMM_HEADING, flag: rommFlag(status), rows };
}

/** Whether `key` is one of the RomM group's keys. */
export function isRommRailKey(key: string): key is RommRailKey {
  return key === 'romm:installed' || key === 'romm:discover' || key.startsWith('platform:');
}

/**
 * The selection after the group changed: `key` itself while it still exists
 * (or is not a RomM key); RomM › Installed when a RomM row disappeared; All
 * games when the whole group is gone.
 */
export function rommSelectionFallback(key: string, group: RommRailGroup | null): string {
  if (!isRommRailKey(key)) return key;
  if (group === null) return 'all';
  if (group.rows.some((r) => r.key === key)) return key;
  return 'romm:installed';
}
