import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import {
  configureGameStores,
  endedStoreRows,
  enabledStatuses,
  findStoreGame,
  gameStores,
  initGameStores,
  onStoreLogin,
  onStoreNotice,
  resetGameStoresForTest,
} from './gameStores.svelte';
import { FakeStores } from '../gamestores/fakeApi';
import { row, status } from '../gamestores/fixtures.test-util';
import type { StoreLibrary } from '../api';

type Handler = (e: { payload: unknown }) => void;

function harness() {
  const handlers = new Map<string, Handler>();
  const fake = new FakeStores((event, payload) => handlers.get(event)?.({ payload }));
  configureGameStores({
    api: {
      storesStatus: async () => fake.handle('stores_status', {}) as never,
      storeLibrary: async (source) => fake.handle('store_library', { source }) as StoreLibrary,
    },
    listen: (async (event: string, h: Handler) => {
      handlers.set(event, h);
      return () => handlers.delete(event);
    }) as never,
  });
  return { fake, handlers };
}

afterEach(() => {
  resetGameStoresForTest();
  vi.useRealTimers();
});

describe('pure helpers', () => {
  it('keeps enabled stores only', () => {
    expect(enabledStatuses([status(), status({ source: 'epic' }), status({ source: 'amazon' })]).map((s) => s.source)).toEqual(['gog', 'epic']);
  });
  it('finds a game in any library', () => {
    const fake = new FakeStores();
    const lib = fake.library();
    expect(findStoreGame({ gog: lib }, 'gog:1001')?.title).toBe('Ashen Tide');
    expect(findStoreGame({ gog: lib }, 'gog:404')).toBeNull();
  });
  it('sees store rows that ended', () => {
    const prev = new Map([[1, 'downloading']]);
    expect(endedStoreRows(prev, [row({ id: 1, status: 'completed' })])).toHaveLength(1);
    expect(endedStoreRows(prev, [row({ id: 1, status: 'installing' })])).toHaveLength(0);
    expect(endedStoreRows(new Map(), [row({ id: 1, status: 'completed' })])).toHaveLength(0);
  });
});

describe('the store', () => {
  it('loads statuses and libraries on start', async () => {
    harness();
    await initGameStores();
    expect(gameStores.loaded).toBe(true);
    expect(gameStores.status('gog')?.connection).toBe('connected');
    expect(gameStores.installed('gog').map((g) => g.title)).toEqual(['Ashen Tide', 'Lanternfall', 'Brightwater']);
    expect(gameStores.available('gog')).toHaveLength(5);
    expect(gameStores.game('gog:1006')?.title).toBe('Marrowind Keep');
    expect(gameStores.updates.map((g) => g.title)).toEqual(['Lanternfall']);
  });

  it('follows stores-changed and refreshes the libraries', async () => {
    vi.useFakeTimers();
    const { fake } = harness();
    await initGameStores();
    fake.scenario('expired');
    expect(gameStores.status('gog')?.connection).toBe('expired');
    await vi.advanceTimersByTimeAsync(200);
    expect(gameStores.available('gog')).toEqual([]);
  });

  it('keeps a failed sign-in’s error and clears it on success', async () => {
    const { handlers } = harness();
    await initGameStores();
    const seen: string[] = [];
    onStoreLogin((e, wasExpired) => seen.push(`${e.outcome}:${wasExpired}`));
    handlers.get('store-login')!({ payload: { source: 'gog', outcome: 'failed', username: '', error: 'GOG returned an error.' } });
    expect(gameStores.loginError('gog')).toBe('GOG returned an error.');
    handlers.get('store-login')!({ payload: { source: 'gog', outcome: 'connected', username: 'sam_plays', error: '' } });
    expect(gameStores.loginError('gog')).toBe('');
    expect(seen).toEqual(['failed:false', 'connected:false']);
  });

  it('hands store notices to subscribers', async () => {
    const { handlers } = harness();
    await initGameStores();
    const got: string[] = [];
    onStoreNotice((n) => got.push(n.message));
    handlers.get('store-notice')!({ payload: { source: 'gog', message: 'gogdl installed', detail: '', error: false, action: null } });
    expect(got).toEqual(['gogdl installed']);
  });
});

describe('the fake', () => {
  it('queues an install and finishes it on tick', () => {
    const events: string[] = [];
    const fake = new FakeStores((e) => events.push(e));
    fake.handle('store_install', { request: { key: 'gog:1007', platform: 'linux', language: 'English', dlc_ids: [], install_dir: '', compat_tool: '' } });
    expect(fake.entries[0]).toMatchObject({ kind: 'store_install', status: 'downloading', source_id: 'gog:1007' });
    for (let i = 0; i < 200 && fake.entries[0].status !== 'completed'; i++) fake.tick();
    expect(fake.entries[0].status).toBe('completed');
    expect(fake.games.find((g) => g.key === 'gog:1007')?.installed).toBe(true);
    expect(events).toContain('store-notice');
  });
  it('applies scenarios and restores the base data', () => {
    const fake = new FakeStores();
    fake.scenario('emptyInstalled');
    expect(fake.library().installed).toEqual([]);
    fake.scenario('normal');
    expect(fake.library().installed).toHaveLength(3);
  });
  it('answers NOT_HANDLED for other commands', async () => {
    const { NOT_HANDLED } = await import('../gamestores/fakeApi');
    expect(new FakeStores().handle('list_installed', {})).toBe(NOT_HANDLED);
  });
});
