// The Library rail's folding groups (RommStorefront README § Library rail ›
// Folding groups; TVRommStorefront README § Library › Folders). Pure: which
// group a rail key belongs to, the remembered list of closed groups
// (`[ui] rail_closed`, one list for desktop and TV mode), a closed heading's
// count and its holds-the-selection words. `stores/uiSettings.svelte.ts`
// keeps the list; the desktop and TV rails render it.
import type { StoreSource } from '../api';
import { STORE_ORDER, storeName } from '../gamestores/marks';

/** A group's id in `rail_closed`: `drives`, a store, or `romm`. */
export type RailGroupId = 'drives' | StoreSource | 'romm';

/** Every group GRID draws, in rail order: Backup drives, then the storefronts by name. */
export const RAIL_GROUP_IDS: readonly RailGroupId[] = ['drives', 'amazon', 'epic', 'gog', 'romm', 'steam'];

/** A group heading's name, sentence case. */
export function railGroupName(id: RailGroupId): string {
  if (id === 'drives') return 'Backup drives';
  if (id === 'romm') return 'RomM';
  return storeName(id);
}

/**
 * The stored list as GRID reads it: a list of strings, else empty (every
 * group open), with no error. Blanks and repeats go; ids GRID doesn't know
 * stay, so writing the list back keeps them.
 */
export function parseRailClosed(raw: unknown): string[] {
  if (!Array.isArray(raw) || raw.some((x) => typeof x !== 'string')) return [];
  const out: string[] = [];
  for (const x of raw as string[]) {
    const id = x.trim();
    if (id !== '' && !out.includes(id)) out.push(id);
  }
  return out;
}

/** Whether a group is closed. Not listed (never seen, a fresh profile) is open. */
export function railGroupClosed(closed: readonly string[], id: RailGroupId): boolean {
  return closed.includes(id);
}

/** The list with `id` closed or open; every other id, known or not, stays. */
export function withRailGroup(closed: readonly string[], id: RailGroupId, isClosed: boolean): string[] {
  const rest = closed.filter((x) => x !== id);
  return isClosed ? [...rest, id] : rest;
}

/** Which group a rail key sits in; `null` for All games, Recent and Updates. */
export function railGroupOf(key: string): RailGroupId | null {
  if (key.startsWith('drive:')) return 'drives';
  if (key.startsWith('romm:') || key.startsWith('platform:')) return 'romm';
  const m = /^store:([a-z]+):/.exec(key);
  if (m && (STORE_ORDER as readonly string[]).includes(m[1])) return m[1] as StoreSource;
  return null;
}

/** A closed heading's count and its tooltip / accessible name. */
export type RailClosedCount = { n: number; label: string };

/**
 * A closed heading's count: the Installed row's number for a store or RomM
 * ("3 installed"), the number of drives for Backup drives ("2 drives").
 * `null` (no count) when Installed shows no number (Steam without its
 * client: the flag says it) or no drive is connected.
 */
export function railClosedCount(id: RailGroupId, n: number | null): RailClosedCount | null {
  if (n === null || !Number.isFinite(n) || n < 0) return null;
  if (id === 'drives') return n === 0 ? null : { n, label: n === 1 ? '1 drive' : `${n} drives` };
  return { n, label: `${n} installed` };
}

/** A closed heading holding the selected row: its tooltip. */
export function railHoldsTitle(id: RailGroupId, rowLabel: string): string {
  return `Showing ${railGroupName(id)} › ${rowLabel}`;
}

/** A closed heading holding the selected row: its accessible description. */
export const RAIL_HOLDS_DESCRIPTION = 'Holds the view you’re in';

/** One item of a rail split into groups: a plain row, or a group and its rows. */
export type RailFoldItem<E> = { kind: 'row'; entry: E } | { kind: 'group'; id: RailGroupId; rows: E[] };

/**
 * The rail's entries with each group's rows gathered under it, in order.
 * Rows of one group are next to each other in the rail (drives, then each
 * storefront), so a run of keys in the same group is one group.
 */
export function foldRailEntries<E extends { key: string }>(entries: readonly E[]): RailFoldItem<E>[] {
  const out: RailFoldItem<E>[] = [];
  for (const entry of entries) {
    const id = railGroupOf(entry.key);
    const last = out[out.length - 1];
    if (id === null) out.push({ kind: 'row', entry });
    else if (last?.kind === 'group' && last.id === id) last.rows.push(entry);
    else out.push({ kind: 'group', id, rows: [entry] });
  }
  return out;
}
