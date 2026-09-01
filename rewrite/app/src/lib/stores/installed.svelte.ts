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
 * Pure matcher (exported for unit tests): a registry row identifies the same
 * game as `game` on this platform if its rom_id matches, else if title and
 * platform match case/whitespace-insensitively (covers rows installed before
 * a rom_id was recorded, or roms re-linked on the server).
 */
export function matchesInstalled(row: InstalledGame, game: GameSummary, platformName: string): boolean {
  if (row.rom_id !== null && row.rom_id === game.id) return true;
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
