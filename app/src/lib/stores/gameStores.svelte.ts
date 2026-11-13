// The external stores' state for every screen (GameStores and TVStores
// cards): each store's status, its owned library (Installed and Available),
// the last failed sign-in, and the `store-notice` / `store-login` events.
// Fed by `stores_status`, `store_library`, `stores-changed`, `store-login`,
// `store-notice`, `installed-changed` and the downloads snapshots (a store
// row that ends refreshes its store's library). UI-agnostic: the desktop and
// TV mode subscribe to the notices and show them their own way.
//
// The API and the event source are injectable (`configureGameStores`) so the
// tests and the dev-only fake (`gamestores/fakeApi.ts`) drive the same code.
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api as realApi,
  STORE_LOGIN_EVENT,
  STORE_NOTICE_EVENT,
  STORES_CHANGED_EVENT,
  type DownloadEntry,
  type StoreGame,
  type StoreLibrary,
  type StoreLoginEvent,
  type StoreNotice,
  type StoreSource,
  type StoreStatus,
} from '../api';
import { ENABLED_STORES } from '../gamestores/marks';
import { storeShown } from '../gamestores/rail';
import { isStoreJob } from '../gamestores/jobs';
import { onSnapshot } from './downloads.svelte';

/** The slice of `api` this store calls. */
export type GameStoresApi = Pick<typeof realApi, 'storesStatus' | 'storeLibrary'>;
export type ListenFn = <T>(event: string, handler: (e: { payload: T }) => void) => Promise<UnlistenFn>;

let deps: { api: GameStoresApi; listen: ListenFn } = { api: realApi, listen: tauriListen as ListenFn };

/** Swaps the API and event source (tests, the dev-only fake). Call before `initGameStores`. */
export function configureGameStores(next: Partial<{ api: GameStoresApi; listen: ListenFn }>): void {
  deps = { ...deps, ...next };
}

type State = {
  statuses: StoreStatus[];
  libraries: Partial<Record<StoreSource, StoreLibrary>>;
  /** The last failed sign-in's error per store (shown on its Settings card). */
  loginErrors: Partial<Record<StoreSource, string>>;
  loaded: boolean;
};

const state = $state<State>({ statuses: [], libraries: {}, loginErrors: {}, loaded: false });

// ── Pure helpers (exported for tests) ──────────────────────────────────

/** Only the stores this build shows. */
export function enabledStatuses(list: readonly StoreStatus[]): StoreStatus[] {
  return list.filter((s) => (ENABLED_STORES as readonly string[]).includes(s.source));
}

/** The game with `key` in any loaded library, installed first. */
export function findStoreGame(libraries: Partial<Record<StoreSource, StoreLibrary>>, key: string): StoreGame | null {
  for (const lib of Object.values(libraries)) {
    if (!lib) continue;
    const hit = lib.installed.find((g) => g.key === key) ?? lib.available.find((g) => g.key === key);
    if (hit) return hit;
  }
  return null;
}

/** The store rows that reached a terminal status since `prev` (they change what is installed). */
export function endedStoreRows(prev: ReadonlyMap<number, string>, entries: readonly DownloadEntry[]): DownloadEntry[] {
  const terminal = new Set(['completed', 'failed', 'cancelled']);
  return entries.filter((e) => isStoreJob(e) && terminal.has(e.status) && prev.has(e.id) && !terminal.has(prev.get(e.id) ?? ''));
}

// ── Reads ──────────────────────────────────────────────────────────────

export const gameStores = {
  get loaded(): boolean {
    return state.loaded;
  },
  /** Every enabled store's status, in the backend's order. */
  get statuses(): StoreStatus[] {
    return state.statuses;
  },
  status(source: StoreSource): StoreStatus | null {
    return state.statuses.find((s) => s.source === source) ?? null;
  },
  library(source: StoreSource): StoreLibrary | null {
    return state.libraries[source] ?? null;
  },
  installed(source: StoreSource): StoreGame[] {
    return state.libraries[source]?.installed ?? [];
  },
  available(source: StoreSource): StoreGame[] {
    return state.libraries[source]?.available ?? [];
  },
  game(key: string): StoreGame | null {
    return findStoreGame(state.libraries, key);
  },
  /** Every installed game of every store that has a rail group (All games, Recent, Updates mix them in). */
  get installedGames(): StoreGame[] {
    return state.statuses.filter(storeShown).flatMap((s) => state.libraries[s.source]?.installed ?? []);
  },
  /** Installed store games with an update (Library › Updates counts them). */
  get updates(): StoreGame[] {
    return this.installedGames.filter((g) => g.update_version !== '');
  },
  loginError(source: StoreSource): string {
    return state.loginErrors[source] ?? '';
  },
};

// ── Writes ─────────────────────────────────────────────────────────────

/** Re-reads one store's library. A failure leaves the last one on screen. */
export async function refreshLibrary(source: StoreSource): Promise<void> {
  try {
    const lib = await deps.api.storeLibrary(source);
    state.libraries = { ...state.libraries, [source]: lib };
    // The library carries the freshest status too.
    applyStatuses(state.statuses.map((s) => (s.source === source ? lib.status : s)));
  } catch {
    // Kept: the store's last library stays.
  }
}

function applyStatuses(list: readonly StoreStatus[]): void {
  state.statuses = enabledStatuses(list);
}

/** Re-reads every status, then the libraries of the stores with a rail group. */
export async function refreshGameStores(): Promise<void> {
  try {
    applyStatuses(await deps.api.storesStatus());
  } catch {
    // No stores readable (an older backend): no store groups.
  }
  state.loaded = true;
  await Promise.all(state.statuses.filter(storeShown).map((s) => refreshLibrary(s.source)));
}

/** Clears the failed sign-in line (a new attempt starts). */
export function clearLoginError(source: StoreSource): void {
  if (!(source in state.loginErrors)) return;
  const { [source]: _gone, ...rest } = state.loginErrors;
  state.loginErrors = rest;
}

const noticeListeners = new Set<(n: StoreNotice) => void>();
const loginListeners = new Set<(e: StoreLoginEvent, wasExpired: boolean) => void>();

/** Every `store-notice` from now on (the desktop's cards, TV mode's cards). */
export function onStoreNotice(fn: (n: StoreNotice) => void): () => void {
  noticeListeners.add(fn);
  return () => noticeListeners.delete(fn);
}

/** Every finished sign-in (`store-login`), with whether the login had expired before. */
export function onStoreLogin(fn: (e: StoreLoginEvent, wasExpired: boolean) => void): () => void {
  loginListeners.add(fn);
  return () => loginListeners.delete(fn);
}

let started: Promise<() => void> | null = null;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;

/** Refreshes the libraries once a burst of events settles. */
function scheduleLibraries(): void {
  if (refreshTimer !== null) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    void Promise.all(state.statuses.filter(storeShown).map((s) => refreshLibrary(s.source)));
  }, 150);
}

/**
 * Starts the store once for the app's life (either shell may call it; the
 * second call shares the first). Answers the function that stops it.
 */
export function initGameStores(): Promise<() => void> {
  if (started) return started;
  started = (async () => {
    const offs: UnlistenFn[] = [];
    offs.push(
      await deps.listen<StoreStatus[]>(STORES_CHANGED_EVENT, (e) => {
        applyStatuses(e.payload);
        state.loaded = true;
        scheduleLibraries();
      }),
    );
    offs.push(
      await deps.listen<StoreLoginEvent>(STORE_LOGIN_EVENT, (e) => {
        const ev = e.payload;
        const wasExpired = state.statuses.find((s) => s.source === ev.source)?.connection === 'expired';
        if (ev.outcome === 'failed') state.loginErrors = { ...state.loginErrors, [ev.source]: ev.error };
        else if (ev.outcome === 'connected') clearLoginError(ev.source);
        for (const fn of loginListeners) fn(ev, wasExpired);
        if (ev.outcome === 'connected') void refreshGameStores();
      }),
    );
    offs.push(
      await deps.listen<StoreNotice>(STORE_NOTICE_EVENT, (e) => {
        for (const fn of noticeListeners) fn(e.payload);
      }),
    );
    offs.push(await deps.listen<unknown>('installed-changed', () => scheduleLibraries()));
    let book = new Map<number, string>();
    const offSnap = onSnapshot((entries) => {
      if (endedStoreRows(book, entries).length > 0) scheduleLibraries();
      book = new Map(entries.map((e) => [e.id, e.status]));
    });
    await refreshGameStores();
    return () => {
      for (const off of offs) off();
      offSnap();
      if (refreshTimer !== null) clearTimeout(refreshTimer);
      started = null;
    };
  })();
  return started;
}

/** Test-only: forget everything. */
export function resetGameStoresForTest(): void {
  state.statuses = [];
  state.libraries = {};
  state.loginErrors = {};
  state.loaded = false;
  started = null;
  noticeListeners.clear();
  loginListeners.clear();
}
