// Select mode on the Library and Server grids (checklist items 94, 439–453;
// README § Game backups › Select mode; tvmode.js `selToggle`, `selPick`,
// `selAll`, `selAct`, `selPaint`, `selHints`). RT turns it on and off (the
// desktop's Select / Done), A picks a cover instead of opening Details, X
// picks every game the grid shows or clears them, Y acts on the picked
// games, RT or B leaves. There is no range pick: Select all covers it.
//
// The picks are the covers' `TvGame.id`s, over the desktop's pure selection
// rules (`backups/selection.ts`). The grid on show registers what it shows,
// the rail row's whole list and what Y does with `registerSelectGrid`;
// nothing here reads a store but the toasts, so the rules are tested without
// a screen. Picks a search hides are kept, as on the desktop and in the
// prototype: the bar counts and sizes every pick, X counts the covers shown,
// Y acts on every pick (consultant, after P13).
import { allPicked, EMPTY_SELECTION, keepOnly, selectAll, toggle, type Selection } from '../backups/selection';
import { sizeText } from '../backups/dialog';
import { pushToast } from '../stores/toasts.svelte';
import type { Hint } from './hints';
import { BACKUP_NEEDS_SERVER } from './pages/details';

/** A game the grid shows, as select mode counts it. */
export type SelGame = { id: string; size: number };

/** What the grid on show lists, and what Y does with the picked games. */
export type SelectGrid = {
  /** The games the grid shows, in its order (a search may narrow it). */
  games: () => readonly SelGame[];
  /** The rail row's whole list, before a search: the picks it may hold. */
  all: () => readonly SelGame[];
  /** The grid is a Backup drive's: Y is "Install or remove". */
  drive: () => boolean;
  /** A server answers (Backup to drive… needs one). */
  connected: () => boolean;
  /** Y with games picked: the Backup page, or the drive's list. */
  act: (ids: string[]) => void;
};

const state = $state<{ on: boolean; sel: Selection<string> }>({ on: false, sel: EMPTY_SELECTION });

let grid: SelectGrid | null = null;

export const select = {
  get on(): boolean {
    return state.on;
  },
  get ids(): readonly string[] {
    return state.sel.ids;
  },
  picked(id: string): boolean {
    return state.on && state.sel.ids.includes(id);
  },
};

/** The grid on show registers itself; answers the function that drops it. */
export function registerSelectGrid(g: SelectGrid): () => void {
  grid = g;
  return () => {
    if (grid === g) grid = null;
  };
}

/** RT: select mode on (with nothing picked, item 439) or off. */
export function selToggle(): void {
  state.on = !state.on;
  state.sel = EMPTY_SELECTION;
}

/** Leaves select mode: acting on the picked games, the drive going away,
 *  another tab or rail row (item 452). Nothing when it is off. */
export function selEnd(): void {
  if (!state.on && state.sel.ids.length === 0) return;
  state.on = false;
  state.sel = EMPTY_SELECTION;
}

/** A on a cover in select mode: picks or unpicks it (item 440). */
export function selPick(id: string): void {
  if (!state.on) return;
  state.sel = toggle(state.sel, id);
}

/** X: every game the grid shows, or none once they all are (item 442). */
export function selAll(visible: readonly string[] = (grid?.games() ?? []).map((g) => g.id)): void {
  if (!state.on) return;
  state.sel = selectAll(state.sel, visible);
}

/** Drops picks the rail row no longer lists (a game left the drive or the
 *  server). Pass the row's whole list: a search that hides a pick keeps it. */
export function selKeep(present: readonly string[]): void {
  const next = keepOnly(state.sel, present);
  if (next !== state.sel) state.sel = next;
}

/** What Y with these picks does (items 443, 447–449). */
export type SelActOutcome = 'pick-first' | 'offline' | 'drive' | 'backup';

export function selActOutcome(picked: number, drive: boolean, connected: boolean): SelActOutcome {
  if (picked === 0) return 'pick-first';
  if (drive) return 'drive';
  if (!connected) return 'offline';
  return 'backup';
}

export const PICK_FIRST = { title: 'Pick games first', sub: 'A picks the focused game; X picks them all.' } as const;
export { BACKUP_NEEDS_SERVER };

/** Y in select mode. */
export function selAct(g: SelectGrid | null = grid): void {
  if (!state.on || !g) return;
  const ids = [...state.sel.ids];
  switch (selActOutcome(ids.length, g.drive(), g.connected())) {
    case 'pick-first':
      pushToast(PICK_FIRST.title, 'success', undefined, { sub: PICK_FIRST.sub });
      return;
    case 'offline':
      pushToast(BACKUP_NEEDS_SERVER, 'error');
      return;
    default:
      g.act(ids);
  }
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** The bar under the grid (item 445). */
export type SelBar = { summary: string; x: string; y: string; rt: string };

/** `shown`: the covers on show (X's count); `whole`: the rail row's whole
 *  list, which sizes every pick, hidden by a search or not. */
export function selBar(sel: readonly string[], shown: readonly SelGame[], whole: readonly SelGame[], drive: boolean): SelBar {
  const ids = shown.map((g) => g.id);
  const all = allPicked({ ids: sel, anchor: null }, ids);
  const bytes = whole.filter((g) => sel.includes(g.id)).reduce((s, g) => s + Math.max(0, g.size || 0), 0);
  return {
    summary: `${sel.length} selected · ${sizeText(bytes)}`,
    x: all ? `All ${ids.length} selected · Clear` : `Select all ${ids.length}`,
    y: drive ? 'Install or remove' : 'Backup to drive…',
    rt: 'Done',
  };
}

/** Select mode's hints, which replace the rest (item 446). `onPicked`: the
 *  focused item is a picked cover; `cover`: it is a cover at all. */
export function selHints(o: { cover: boolean; onPicked: boolean; a: string; all: boolean; drive: boolean }): Hint[] {
  return [
    { button: 'A', label: o.cover ? (o.onPicked ? 'Unpick' : 'Pick') : o.a },
    { button: 'X', label: o.all ? 'Clear all' : 'Select all' },
    { button: 'Y', label: o.drive ? 'Install or remove' : 'Backup to drive…' },
    { button: 'RT', label: 'Done' },
  ];
}

/** The hint list for the grid on show, or null when select mode is off. */
export function currentSelHints(focused: HTMLElement | null, a: string): Hint[] | null {
  if (!state.on || !grid) return null;
  const ids = grid.games().map((g) => g.id);
  const id = focused?.getAttribute('data-sel') ?? null;
  return selHints({
    cover: id !== null,
    onPicked: id !== null && state.sel.ids.includes(id),
    a,
    all: allPicked(state.sel, ids),
    drive: grid.drive(),
  });
}
