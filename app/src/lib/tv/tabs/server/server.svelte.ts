// The Server tab's state that outlives its screen (TvStage mounts one screen
// at a time, so the tab unmounts on every tab switch): the selected rail row,
// the platform search, the Discover cover last focused, and the catalog
// (tvmode.js `S.srvRail`, `S.srvQ`, `TABS.server._dk`). Kept per server.
import { api, type GameSummary } from '../../../api';
import { Catalog } from '../../../server/catalog.svelte';
import { platformLabel } from '../../../server/platformLabel';
import { session } from '../../../stores/session.svelte';
import { rememberFocus, showTab } from '../../nav.svelte';

export type SrvRail = 'discover' | number;

export const srv = $state<{
  rail: SrvRail;
  /** The platform search (Y); a new platform clears it (item 204). */
  query: string;
  /** Discover's cover last focused (item 104), by `data-k`. */
  discoverKey: string | null;
  /** Server was arrived on before Discover had covers: the first one to
   *  appear takes focus (item 417). */
  pendingArrive: boolean;
}>({ rail: 'discover', query: '', discoverKey: null, pendingArrive: false });

let catalog: Catalog | null = null;
let catalogServer = '';

/** The catalog of the server GRID is connected to (fresh for another one). */
export function serverCatalog(): Catalog {
  if (!catalog || catalogServer !== session.serverUrl) {
    catalog = new Catalog();
    catalogServer = session.serverUrl;
    allGames = null;
    srv.rail = 'discover';
    srv.query = '';
    srv.discoverKey = null;
  }
  return catalog;
}

/** Selects a rail row; a new one clears the platform search (item 204). */
export function selectRail(rail: SrvRail): void {
  if (srv.rail === rail) return;
  srv.rail = rail;
  srv.query = '';
  if (rail !== 'discover') srv.pendingArrive = false;
}

/**
 * Menu › Discover (item 365; tvmode.js `guideMenu`): any pages close, the
 * Server tab shows with Discover selected, and its `arrive` carries focus
 * from the Discover row into Discover's content.
 */
export async function goDiscover(): Promise<void> {
  selectRail('discover');
  rememberFocus('tab:server', 'r-discover');
  await showTab('server', { again: true });
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
  const server = session.serverUrl;
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
