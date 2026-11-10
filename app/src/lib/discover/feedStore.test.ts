import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DiscoverFeed, GameSummary } from '../api';

type Handler = () => void;

const feed = (id: number) => ({ new_releases: [{ id }], recently_added: [], recent_platforms: [] }) as unknown as DiscoverFeed;
const games = (...ids: number[]) => ids.map((id) => ({ id }) as GameSummary);

describe('feedStore', () => {
  let api: Record<string, ReturnType<typeof vi.fn>>;
  let handlers: Record<string, Handler>;
  let unlistened: string[];

  beforeEach(() => {
    vi.resetModules();
    handlers = {};
    unlistened = [];
    api = {
      discoverFeed: vi.fn(async () => feed(1)),
      discoverRecentlyAdded: vi.fn(async (id: number) => games(id * 10)),
    };
    vi.doMock('../api', () => ({
      api,
      EMULATOR_INSTALLED_EVENT: 'emulator-installed',
      SETUP_DEFAULTS_EVENT: 'setup-defaults',
    }));
    vi.doMock('@tauri-apps/api/event', () => ({
      listen: (name: string, handler: Handler) => {
        handlers[name] = handler;
        return Promise.resolve(() => unlistened.push(name));
      },
    }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('@tauri-apps/api/event');
  });

  it('keeps the last feed shown, per server', async () => {
    const store = await import('./feedStore.svelte');
    expect(store.cachedFeed('https://a')).toBeNull();
    const next = await store.fetchFeed();
    // Fetching alone keeps nothing: only a feed that was shown is remembered.
    expect(store.cachedFeed('https://a')).toBeNull();
    store.rememberFeed('https://a', next);
    expect(store.cachedFeed('https://a')).toBe(next);
    expect(store.cachedFeed('https://b')).toBeNull();
    store.forgetFeed();
    expect(store.cachedFeed('https://a')).toBeNull();
  });

  it('fetches a platform list once per feed', async () => {
    const { PlatformLists } = await import('./feedStore.svelte');
    const lists = new PlatformLists();
    expect(lists.cached(3)).toBeUndefined();
    expect(await lists.load(3, [1, 2])).toEqual(games(30));
    expect(api.discoverRecentlyAdded).toHaveBeenCalledWith(3, [1, 2]);
    expect(lists.cached(3)).toEqual(games(30));
    lists.reset();
    expect(lists.cached(3)).toBeUndefined();
  });

  it('keeps an answer from before a reset out of the new cache', async () => {
    const { PlatformLists } = await import('./feedStore.svelte');
    const lists = new PlatformLists();
    let release!: (g: GameSummary[]) => void;
    api.discoverRecentlyAdded.mockReturnValueOnce(new Promise((res) => (release = res)));
    const pending = lists.load(4, []);
    lists.reset();
    release(games(40));
    expect(await pending).toEqual(games(40));
    expect(lists.cached(4)).toBeUndefined();
  });

  it('reports a change of the feed inputs until stopped', async () => {
    const { watchFeedInputs } = await import('./feedStore.svelte');
    const onChange = vi.fn();
    const stop = watchFeedInputs(onChange);
    handlers['emulator-installed']();
    handlers['setup-defaults']();
    expect(onChange).toHaveBeenCalledTimes(2);
    stop();
    await Promise.resolve();
    expect(unlistened.sort()).toEqual(['emulator-installed', 'setup-defaults']);
  });
});
