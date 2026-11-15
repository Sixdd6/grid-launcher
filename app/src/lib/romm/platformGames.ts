// A RomM platform view's grid (plan 0.3, decisions 14 and 15): the server's
// games for the platform, each matched to its installed row, plus the
// installed rows the server doesn't have, tagged NOT LINKED or NOT ON ROMM.
// Pure and O(n) (a Map by rom id), so a 2000-game platform stays fast.
import type { GameSummary, InstalledGame, Platform } from '../api';
import { platformKeyer } from './platformKey';
import { rommInstalledRows } from './rail';
import { ROMM_COVER_TAGS } from './words';

/** An installed row the server doesn't have: no RomM link, or from another
 *  server / gone from this one. */
export type LocalTag = 'not_linked' | 'not_on_romm';

export type RommGridItem =
  /** A server game; `installed` is its installed row (Play), or `null` (Install). */
  | { kind: 'server'; game: GameSummary; installed: InstalledGame | null }
  /** An installed row only on this computer. */
  | { kind: 'local'; row: InstalledGame; tag: LocalTag };

/** A server URL reduced for comparison: trimmed, no trailing slash, lower case. */
export function normalizeServer(url: string): string {
  return url.trim().replace(/\/+$/, '').toLowerCase();
}

/** Whether an installed row came from the live server (its `source_server`
 *  is empty, or equals the live server). */
export function fromLiveServer(row: Pick<InstalledGame, 'source_server'>, liveServer: string): boolean {
  const own = normalizeServer(row.source_server ?? '');
  return own === '' || own === normalizeServer(liveServer);
}

/**
 * The installed RomM rows of the platform `slug` (key rule in
 * `platformKey.ts`), against the server's platform list.
 */
export function installedOnPlatform(
  installedRows: readonly InstalledGame[],
  slug: string,
  serverPlatforms: readonly Platform[],
): InstalledGame[] {
  const keyOf = platformKeyer(serverPlatforms);
  return rommInstalledRows(installedRows).filter((r) => keyOf(r) === slug);
}

/**
 * The merged grid. `serverGames`: the server's games for the platform, in
 * the server view's order. `installedRows`: the platform's installed RomM
 * rows (`installedOnPlatform`). `liveServer`: the connected server's URL.
 * Server games come first in their order, then the local-only rows in theirs.
 */
export function platformGames(
  serverGames: readonly GameSummary[],
  installedRows: readonly InstalledGame[],
  liveServer: string,
): RommGridItem[] {
  const byRomId = new Map<number, InstalledGame>();
  for (const row of installedRows) {
    if (row.rom_id !== null && fromLiveServer(row, liveServer) && !byRomId.has(row.rom_id)) byRomId.set(row.rom_id, row);
  }
  const matched = new Set<InstalledGame>();
  const items: RommGridItem[] = serverGames.map((game) => {
    const installed = byRomId.get(game.id) ?? null;
    if (installed !== null) matched.add(installed);
    return { kind: 'server', game, installed };
  });
  for (const row of installedRows) {
    if (matched.has(row)) continue;
    items.push({ kind: 'local', row, tag: row.rom_id === null ? 'not_linked' : 'not_on_romm' });
  }
  return items;
}

/** The counts line's numbers: games on RomM, of those installed, and installed rows only here. */
export function platformGameCounts(items: readonly RommGridItem[]): { onRomm: number; installed: number; onlyHere: number } {
  let onRomm = 0;
  let installed = 0;
  let onlyHere = 0;
  for (const item of items) {
    if (item.kind === 'server') {
      onRomm += 1;
      if (item.installed !== null) installed += 1;
    } else {
      onlyHere += 1;
    }
  }
  return { onRomm, installed, onlyHere };
}

/** The cover tag's words for a local-only row. */
export function localTagLabel(tag: LocalTag): string {
  return ROMM_COVER_TAGS[tag];
}
