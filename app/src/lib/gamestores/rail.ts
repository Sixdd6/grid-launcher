// The Library rail's store groups (GameStores README, "Library › Rail group
// per store"): under Backup drives, one group per store with its mark as the
// heading, then Installed and Available with counts. Pure: Library.svelte and
// TV mode render these.
import type { StoreLibrary, StoreSource, StoreStatus } from '../api';
import { ENABLED_STORES, STORE_ORDER, storeName } from './marks';

export type StorePart = 'installed' | 'available';

/** A store row's rail key: `store:gog:installed`. */
export type StoreRailKey = `store:${StoreSource}:${StorePart}`;

export function storeRailKey(source: StoreSource, part: StorePart): StoreRailKey {
  return `store:${source}:${part}`;
}

/** The store and part a `store:` rail key names, or `null` for any other key. */
export function parseStoreRailKey(key: string): { source: StoreSource; part: StorePart } | null {
  const m = /^store:([a-z]+):(installed|available)$/.exec(key);
  if (!m) return null;
  if (!(STORE_ORDER as readonly string[]).includes(m[1])) return null;
  return { source: m[1] as StoreSource, part: m[2] as StorePart };
}

/**
 * Why Available can't list games right now (the prototype's `stAvailBlock`),
 * or `''` when it can. `error` is an owned list that failed to load.
 */
export type AvailBlock = '' | 'off' | 'expired' | 'tool' | 'loading' | 'error';

export function availBlock(status: StoreStatus): AvailBlock {
  if (status.connection === 'not_connected') return 'off';
  if (status.connection === 'expired') return 'expired';
  if (status.tool.state !== 'installed') return 'tool';
  if (status.owned === 'loading' || status.owned === 'idle') return 'loading';
  if (status.owned === 'error') return 'error';
  return '';
}

/**
 * Whether the store has a rail group: connected, login expired, or with games
 * still installed here after a Disconnect (or an offline install with no
 * login). The backend says so in `in_rail`; the rest is a fallback for a
 * status from before that field was filled.
 */
export function storeShown(status: StoreStatus): boolean {
  return status.in_rail || status.connection !== 'not_connected' || status.installed_count > 0;
}

/** The heading's badge: `!` when the store needs attention, `off` when it isn't connected. */
export type StoreFlag = { text: '!' | 'off'; title: string } | null;

export function storeFlag(status: StoreStatus): StoreFlag {
  if (status.connection === 'expired') return { text: '!', title: 'Login expired' };
  if (status.connection === 'not_connected') return { text: 'off', title: 'Not connected' };
  if (status.tool.state !== 'installed') return { text: '!', title: 'Needs attention' };
  return null;
}

/** A row's count: a number, a spinner while the owned list loads, or `!`. */
export type StoreCount = { kind: 'number'; n: number } | { kind: 'loading' } | { kind: 'attention' };

export type StoreRailRow = {
  key: StoreRailKey;
  part: StorePart;
  label: 'Installed' | 'Available';
  count: StoreCount;
  /** `library-rail-store-gog-installed`. */
  testId: string;
};

export type StoreRailGroup = {
  source: StoreSource;
  /** The heading, the store's name in capitals ("GOG"). */
  heading: string;
  flag: StoreFlag;
  rows: [StoreRailRow, StoreRailRow];
};

/** How many games each part holds: the loaded library when there is one, else the status's counts. */
export function partCounts(status: StoreStatus, library: StoreLibrary | null): { installed: number; available: number } {
  if (library !== null) return { installed: library.installed.length, available: library.available.length };
  return {
    installed: status.installed_count,
    available: Math.max(0, status.owned_count - status.installed_count),
  };
}

function availableCount(status: StoreStatus, n: number): StoreCount {
  const block = availBlock(status);
  if (block === 'loading') return { kind: 'loading' };
  if (block !== '') return { kind: 'attention' };
  return { kind: 'number', n };
}

/**
 * The rail's store groups in rail order (alphabetical by name), only the
 * stores this build shows and only those `storeShown` keeps.
 */
export function storeRailGroups(
  statuses: readonly StoreStatus[],
  libraryOf: (source: StoreSource) => StoreLibrary | null = () => null,
): StoreRailGroup[] {
  const out: StoreRailGroup[] = [];
  for (const source of STORE_ORDER) {
    if (!(ENABLED_STORES as readonly string[]).includes(source)) continue;
    const status = statuses.find((s) => s.source === source);
    if (status === undefined || !storeShown(status)) continue;
    const counts = partCounts(status, libraryOf(source));
    out.push({
      source,
      heading: storeName(source).toUpperCase(),
      flag: storeFlag(status),
      rows: [
        {
          key: storeRailKey(source, 'installed'),
          part: 'installed',
          label: 'Installed',
          count: { kind: 'number', n: counts.installed },
          testId: `library-rail-store-${source}-installed`,
        },
        {
          key: storeRailKey(source, 'available'),
          part: 'available',
          label: 'Available',
          count: availableCount(status, counts.available),
          testId: `library-rail-store-${source}-available`,
        },
      ],
    });
  }
  return out;
}
