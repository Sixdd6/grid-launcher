// TV mode's RomM state that outlives a screen (TvStage remounts a tab on
// every switch; TVRommStorefront README § Library › RomM group): which RomM
// row of the Library rail is on show, its folder, the platform search, the
// Discover cover last focused, and the server catalog (platforms, a
// platform's games). The status itself is the shared `romm` store.
import { api, type GameSummary } from '../../api';
import type { RommRailKey } from '../../romm/rail';
import { Catalog } from '../../server/catalog.svelte';
import { platformLabel } from '../../server/platformLabel';
import { romm } from '../../stores/romm.svelte';
import { rememberFocus, showTab } from '../nav.svelte';
import { selEnd } from '../select.svelte';
import { clearStoreView } from '../stores/storeState.svelte';

const state = $state<{
  /** The Library's RomM row on show, or null for another rail row. */
  view: RommRailKey | null;
  /** The RomM folder, open by default (README: a folder like a store's). */
  open: boolean;
  /** The search of RomM › Installed or a platform (Y); a new row clears it. */
  query: string;
  /** Discover's cover last focused (item 104), by `data-k`. */
  discoverKey: string | null;
  /** The Library was arrived on before Discover had covers: the first one
   *  to appear takes focus (item 417). */
  pendingArrive: boolean;
}>({ view: null, open: true, query: '', discoverKey: null, pendingArrive: false });

export const tvRomm = {
  get view(): RommRailKey | null {
    return state.view;
  },
  get open(): boolean {
    return state.open;
  },
  get query(): string {
    return state.query;
  },
  set query(v: string) {
    state.query = v;
  },
  get discoverKey(): string | null {
    return state.discoverKey;
  },
  set discoverKey(k: string | null) {
    state.discoverKey = k;
  },
  get pendingArrive(): boolean {
    return state.pendingArrive;
  },
  set pendingArrive(on: boolean) {
    state.pendingArrive = on;
  },
};

/** The `data-k` of a RomM rail row (`r-romm:installed`, `r-platform:psx`). */
export function rommRowKey(key: RommRailKey): string {
  return `r-${key}`;
}

/** The RomM folder header's `data-k`. */
export const ROMM_FOLD_KEY = 'fold-romm';

/** Moving onto a RomM row shows it at once; a new row clears the search and
 *  leaves select mode. Store views end. */
export function selectRommView(key: RommRailKey): void {
  clearStoreView();
  if (state.view === key) return;
  selEnd();
  state.view = key;
  state.query = '';
  if (key !== 'romm:discover') state.pendingArrive = false;
}

/** Any other rail row (All games, a drive, a store's row) leaves the RomM view. */
export function clearRommView(): void {
  state.view = null;
}

export function toggleRommFold(): void {
  state.open = !state.open;
}

/**
 * Jumps the Library to a RomM row (Menu › Discover, Show in Library, the
 * start rule): pages close, the folder opens, the row is selected and
 * focused when the tab shows. Discover's `arrive` then carries focus into
 * its content (item 104).
 */
export async function showRommRow(key: RommRailKey): Promise<void> {
  selectRommView(key);
  state.open = true;
  rememberFocus('tab:library', rommRowKey(key));
  await showTab('library', { again: true });
}

/** Menu › Discover (README § The shell). */
export function goDiscover(): Promise<void> {
  return showRommRow('romm:discover');
}

// ---- the server catalog -------------------------------------------------

let catalog: Catalog | null = null;
let catalogServer = '';

/** The catalog of the RomM server GRID knows (fresh for another one). */
export function rommCatalog(): Catalog {
  const server = romm.status.server_url;
  if (!catalog || catalogServer !== server) {
    catalog = new Catalog();
    catalogServer = server;
    allGames = null;
    state.query = '';
    state.discoverKey = null;
  }
  return catalog;
}

/** A server game with the label its platform shows. */
export type LabelledGame = { game: GameSummary; platform: string; slug: string };

/** Every game a load reached, and whether some platform failed. */
export type AllGames = { games: LabelledGame[]; partial: boolean };

let allGames: { server: string; load: Promise<AllGames> } | null = null;

/**
 * Every game on the server, for Search everything: each platform's list,
 * fetched once per server and session. A platform that fails to load is
 * left out (`partial`); a failed or partial load is dropped from the cache,
 * so the next load (Try again) asks again. Rejects when the platform list
 * itself can't be read: nothing could be searched.
 */
export function loadAllGames(): Promise<AllGames> {
  const server = romm.status.server_url;
  if (allGames && allGames.server === server) return allGames.load;
  const entry: { server: string; load: Promise<AllGames> } = { server, load: Promise.resolve({ games: [], partial: false }) };
  entry.load = (async () => {
    const platforms = await api.listPlatforms();
    let failed = false;
    const lists = await Promise.all(
      platforms.map((p) =>
        api.listGames(p.id).then(
          (games) => games.map((game) => ({ game, platform: platformLabel(p), slug: p.slug })),
          () => {
            failed = true;
            return [] as LabelledGame[];
          },
        ),
      ),
    );
    if (failed && allGames === entry) allGames = null;
    return { games: lists.flat(), partial: failed };
  })();
  allGames = entry;
  entry.load.catch(() => {
    if (allGames === entry) allGames = null;
  });
  return entry.load;
}

/** Tests only. */
export function resetTvRommForTest(): void {
  state.view = null;
  state.open = true;
  state.query = '';
  state.discoverKey = null;
  state.pendingArrive = false;
  catalog = null;
  catalogServer = '';
  allGames = null;
}
