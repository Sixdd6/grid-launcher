// Server › Discover's rows and hero (checklist items 160–169; tvmode.js
// `discoverHTML`, `hero`, `ago`). Pure: TvDiscover feeds it the installed
// games, the feed's New releases (the last six months, on the preferred
// platforms, newest first: `grid_core::romm::discover`) and Recently added,
// and renders what it answers.
import { formatDay } from '../../../discover/feed';
import type { LiveInstall, TvGame } from '../../games';

export type ShelfId = 'c' | 'n' | 'r' | 'u' | 'l';

export type Shelf = {
  id: ShelfId;
  name: string;
  /** The muted text after the name, or null. */
  extra: string | null;
  games: TvGame[];
};

/** An empty row (item 169). */
export const EMPTY_SHELF = 'Nothing here yet.';

/** "1 game", "3 games". */
export function gamesWord(n: number): string {
  return n === 1 ? '1 game' : `${n} games`;
}

/**
 * The rows, in order (item 164): Continue playing (installed games with play
 * history, most recently played first); New releases; Recently added
 * (leaving out what New releases shows); Updates (installed games with an
 * update), only when there are any; Your library (every installed game, in
 * the order given).
 */
export function discoverShelves(input: { library: readonly TvGame[]; newReleases: readonly TvGame[]; recentlyAdded: readonly TvGame[] }): Shelf[] {
  const lib = [...input.library];
  const cont = lib.filter((g) => g.lastPlayedAt > 0).sort((a, b) => b.lastPlayedAt - a.lastPlayedAt);
  const nr = [...input.newReleases];
  const inNew = new Set(nr.map((g) => g.id));
  const ra = input.recentlyAdded.filter((g) => !inNew.has(g.id));
  const up = lib.filter((g) => g.update);
  const shelves: Shelf[] = [
    { id: 'c', name: 'Continue playing', extra: null, games: cont },
    { id: 'n', name: 'New releases', extra: 'last six months', games: nr },
    { id: 'r', name: 'Recently added', extra: 'to your server', games: ra },
  ];
  if (up.length) shelves.push({ id: 'u', name: 'Updates', extra: gamesWord(up.length), games: up });
  shelves.push({ id: 'l', name: 'Your library', extra: gamesWord(lib.length), games: lib });
  return shelves;
}

/** "today", "yesterday", "<n> days ago" under 14 days, else "<n> weeks ago" (item 162). */
export function ago(days: number): string {
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 14) return `${days} days ago`;
  return `${Math.round(days / 7)} weeks ago`;
}

/** Whole days between an epoch-seconds stamp and now. */
export function daysSince(atSeconds: number, nowSeconds: number): number {
  return Math.max(0, Math.floor((nowSeconds - atSeconds) / 86_400));
}

/**
 * The hero's state line (item 161): "Files not found" for a missing game;
 * "Played <when> · <emulator>" ("Installed · …" when never played, "No
 * emulator" when none) for an installed one; "Installing… <n>%" or
 * "Queued" while installing; else "Released <d Mon yyyy> · Not installed".
 */
export function heroLine(game: TvGame, live: LiveInstall | null, emulator: string | null, nowSeconds: number): string {
  if (game.missing) return 'Files not found';
  if (game.installed) {
    const played = game.lastPlayedAt > 0 ? `Played ${ago(daysSince(game.lastPlayedAt, nowSeconds))}` : 'Installed';
    return `${played} · ${emulator || 'No emulator'}`;
  }
  if (live) return live.queued ? 'Queued' : `Installing… ${live.pct}%`;
  return game.releasedAt !== null ? `Released ${formatDay(game.releasedAt * 1000)} · Not installed` : 'Not installed';
}

/** The hero's two lines of summary: the first paragraph. */
export function heroSummary(text: string): string {
  return (text.split('\n').find((l) => l.trim() !== '') ?? '').trim();
}

/**
 * The emulator the hero names for an installed game, from Details' launch
 * line (`launchTargetLine`): "No default emulator" reads "No emulator"; a
 * native game names its compatibility tool on Windows, else "Native"
 * (tvmode.js `emuFor`).
 */
export function heroEmulator(launchLine: string, windows: boolean, compatTool: string): string | null {
  if (launchLine === '') return windows ? compatTool.trim() || 'Proton' : 'Native';
  if (launchLine === 'No default emulator') return null;
  return launchLine;
}
