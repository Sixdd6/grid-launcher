// Server › Discover's data: the feed (New releases and Recently added) and
// Recently added for one platform. Both shells' Discover screens read it.
//
// The last feed shown is kept for the life of the app, per server, so a
// Discover that mounts again (it unmounts while a platform is selected, and
// on a switch of shell) shows it at once while the fresh one loads.
import { listen } from '@tauri-apps/api/event';
import { api, EMULATOR_INSTALLED_EVENT, SETUP_DEFAULTS_EVENT, type DiscoverFeed, type GameSummary } from '../api';

let lastFeed: { server: string; feed: DiscoverFeed } | null = null;

/** The last feed shown for `server`, or `null`. */
export function cachedFeed(server: string): DiscoverFeed | null {
  return lastFeed !== null && lastFeed.server === server ? lastFeed.feed : null;
}

/** Fetches the feed. The caller keeps it with `rememberFeed` once it shows it. */
export function fetchFeed(): Promise<DiscoverFeed> {
  return api.discoverFeed();
}

/** Keeps `feed` as the last one shown for `server`. */
export function rememberFeed(server: string, feed: DiscoverFeed): void {
  lastFeed = { server, feed };
}

/** Forgets the kept feed (tests). */
export function forgetFeed(): void {
  lastFeed = null;
}

/**
 * Recently added for one platform, fetched once per feed. `reset` starts a
 * new generation when a new feed arrives (the coverflow games each list
 * leaves out may have changed); an answer for an older generation is kept
 * only in that older cache, never in the new one.
 */
export class PlatformLists {
  #cache = new Map<number, GameSummary[]>();

  reset(): void {
    this.#cache = new Map();
  }

  cached(id: number): GameSummary[] | undefined {
    return this.#cache.get(id);
  }

  /** Fetches platform `id`'s list, leaving out `exclude` (the coverflow's games). */
  async load(id: number, exclude: number[]): Promise<GameSummary[]> {
    const forCache = this.#cache;
    const games = await api.discoverRecentlyAdded(id, exclude);
    forCache.set(id, games);
    return games;
  }
}

/**
 * Calls `onChange` whenever the feed's inputs change: an emulator install
 * or first-run setup's defaults move the preferred platforms. Answers the
 * function that stops listening.
 */
export function watchFeedInputs(onChange: () => void): () => void {
  const offs = [listen(EMULATOR_INSTALLED_EVENT, onChange), listen(SETUP_DEFAULTS_EVENT, onChange)];
  return () => {
    for (const off of offs) void off.then((un) => un());
  };
}
