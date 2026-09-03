import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadEntry, type GameSummary, type InstalledGame } from '../api';
import { downloads } from './downloads.svelte';

const state = $state<{ list: InstalledGame[] }>({ list: [] });

export const installed = {
  get list() {
    return state.list;
  },
};

export async function refresh(): Promise<void> {
  state.list = await api.listInstalled();
}

/**
 * The Tauri backend emits `images-replenished` after each cover/screenshot
 * replenish job finishes (whether it found anything new or not) — refresh
 * the registry so the Library grid picks up any covers that were missing at
 * install time and have since been backfilled.
 */
export function initReplenishListener(): Promise<UnlistenFn> {
  return listen('images-replenished', () => {
    refresh();
  });
}

/**
 * Pure matcher (exported for unit tests). Per docs/porting/03-library-install.md's
 * identity rules ("if both sides have a non-empty rom_id, compare rom ids;
 * otherwise compare the identity key"): when the row has a rom_id, that
 * comparison is authoritative — a mismatch means NOT installed, with no
 * identity fallback (this is what prevents a wrong-game badge/uninstall in
 * libraries with duplicate titles). The identity fallback (case/whitespace-
 * insensitive title+platform) applies only when the row's rom_id is null
 * (covers rows installed before a rom_id was recorded).
 */
export function matchesInstalled(row: InstalledGame, game: GameSummary, platformName: string): boolean {
  if (row.rom_id !== null) return row.rom_id === game.id;
  return (
    row.title.trim().toLowerCase() === game.name.trim().toLowerCase() &&
    row.platform.trim().toLowerCase() === platformName.trim().toLowerCase()
  );
}

export function isInstalled(game: GameSummary, platformName: string): boolean {
  return state.list.some((row) => matchesInstalled(row, game, platformName));
}

const previousStatuses = new Map<number, DownloadEntry['status']>();

/**
 * Exported so the transition-watching logic itself is a plain, callable
 * function: refreshes the installed registry whenever any entry in the given
 * snapshot just transitioned into 'completed' (i.e. an install finished).
 * Wired reactively below via an internal $effect.root over the downloads
 * store's entries, so nothing outside this module has to remember to call it.
 */
export function watchDownloads(entries: DownloadEntry[]): void {
  let justCompleted = false;
  const seen = new Set<number>();
  for (const entry of entries) {
    seen.add(entry.id);
    const prev = previousStatuses.get(entry.id);
    if (entry.status === 'completed' && prev !== 'completed') justCompleted = true;
    previousStatuses.set(entry.id, entry.status);
  }
  for (const id of [...previousStatuses.keys()]) {
    if (!seen.has(id)) previousStatuses.delete(id);
  }
  if (justCompleted) refresh();
}

// Module-scoped root effect: keeps the installed registry in sync with the
// downloads store for the lifetime of the app, without requiring App.svelte
// or Library.svelte to remember to wire it up.
$effect.root(() => {
  $effect(() => {
    watchDownloads(downloads.entries);
  });
});
