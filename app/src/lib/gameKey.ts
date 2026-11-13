// A game's identity across launch, sessions and the queue, mirroring
// grid-core's `stores::GameKey`: `romm:<rom_id>` for a RomM game,
// `<store>:<store_id>` for a store product (`gog:1207664643`). The string
// form is the IPC form (`launch_game_key`, `GameSession.key`).

import type { GameSource } from './api';

/** The store sources (every `GameSource` but 'romm'). */
export const STORE_SOURCES: readonly GameSource[] = ['gog', 'epic', 'amazon', 'steam'];

/** The fields `gameKey` reads from an installed row. */
export type KeyedRow = {
  source?: GameSource | string | null;
  store_id?: string | null;
  rom_id: number | null;
};

/** Whether `row` came from an external store. A row with no `source` is a RomM row. */
export function isStoreRow(row: Pick<KeyedRow, 'source'>): boolean {
  const source = row.source ?? 'romm';
  return source !== 'romm' && source !== '';
}

/**
 * The row's game key, or `null` when it has none: a RomM row with no rom id
 * (not linked to this server yet), or a store row with a blank store id.
 */
export function gameKey(row: KeyedRow): string | null {
  if (isStoreRow(row)) {
    const id = (row.store_id ?? '').trim();
    return id ? `${row.source}:${id}` : null;
  }
  return row.rom_id === null || row.rom_id === undefined ? null : `romm:${row.rom_id}`;
}

/** A parsed game key. */
export type ParsedGameKey =
  | { source: 'romm'; romId: number }
  | { source: Exclude<GameSource, 'romm'>; storeId: string };

/** `key` split into its parts, or `null` when it is not a game key. Only the first `:` separates. */
export function parseGameKey(key: string): ParsedGameKey | null {
  const at = key.indexOf(':');
  if (at <= 0) return null;
  const source = key.slice(0, at);
  const id = key.slice(at + 1).trim();
  if (!id) return null;
  if (source === 'romm') {
    if (!/^-?\d+$/.test(id)) return null;
    return { source: 'romm', romId: Number(id) };
  }
  if (!(STORE_SOURCES as readonly string[]).includes(source)) return null;
  return { source: source as Exclude<GameSource, 'romm'>, storeId: id };
}
