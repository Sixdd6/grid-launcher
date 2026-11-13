// TV mode's store state that outlives a screen (TvStage remounts a tab on
// every switch): which store view the Library shows, its folders, the cloud
// syncs running, and each drive's GOG games as last read. The stores' own
// data is the shared `gameStores` store (E1).
import type { DriveStoreGame, StoreSource } from '../../api';
import { parseStoreRailKey, storeRailKey, type StorePart, type StoreRailKey } from '../../gamestores/rail';
import { STORE_ORDER } from '../../gamestores/marks';

const state = $state<{
  /** The Library's store row on show, or null for the desktop's rail rows. */
  view: StoreRailKey | null;
  /** Each store folder, open by default (designer decision 1). */
  folds: Record<StoreSource, boolean>;
  /** Game keys whose cloud saves are syncing now (Sync now). */
  syncing: string[];
  /** Bumped when a drive's GOG games may have changed (a restore, a backup, a removal). */
  driveVersion: number;
}>({
  view: null,
  folds: Object.fromEntries(STORE_ORDER.map((s) => [s, true])) as Record<StoreSource, boolean>,
  syncing: [],
  driveVersion: 0,
});

export const tvStores = {
  get view(): { source: StoreSource; part: StorePart } | null {
    return state.view === null ? null : parseStoreRailKey(state.view);
  },
  get viewKey(): StoreRailKey | null {
    return state.view;
  },
  folded(source: StoreSource): boolean {
    return !state.folds[source];
  },
  syncing(key: string): boolean {
    return state.syncing.includes(key);
  },
  get driveVersion(): number {
    return state.driveVersion;
  },
};

/** Moving onto a store row shows that list. */
export function selectStoreView(source: StoreSource, part: StorePart): void {
  state.view = storeRailKey(source, part);
}

/** Moving onto any other rail row leaves the store view. */
export function clearStoreView(): void {
  state.view = null;
}

export function toggleStoreFold(source: StoreSource): void {
  state.folds[source] = !state.folds[source];
}

export function openStoreFold(source: StoreSource): void {
  state.folds[source] = true;
}

export function setSyncing(key: string, on: boolean): void {
  state.syncing = on ? [...state.syncing.filter((k) => k !== key), key] : state.syncing.filter((k) => k !== key);
}

export function bumpDrives(): void {
  state.driveVersion++;
}

/** Each drive's GOG games as last read (not reactive: a cache). */
export const driveStoreCache = new Map<string, DriveStoreGame[]>();

/** The rail row keys TV mode gives a store row (`r-store:gog:installed`) and a store folder (`fold-store-gog`). */
export function storeRowKey(source: StoreSource, part: StorePart): string {
  return `r-${storeRailKey(source, part)}`;
}
export function storeFoldKey(source: StoreSource): string {
  return `fold-store-${source}`;
}
