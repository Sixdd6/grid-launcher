import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import {
  configureRomm,
  connectRomm,
  disconnectRomm,
  phaseAfterConnect,
  phaseAfterWelcome,
  reconnectRomm,
  resetRommForTest,
  romm,
  type RommApi,
} from './romm.svelte';
import { session } from './session.svelte';
import { rommStatus } from '../romm/fixtures.test-util';
import type { RommStatus } from '../api';

function fakeApi(over: Partial<RommApi> = {}): RommApi {
  const status = rommStatus();
  return {
    rommStatus: async () => status,
    rommRestore: async () => status,
    rommConnect: async () => status,
    rommReconnect: async () => status,
    rommDisconnect: async () => rommStatus({ connection: 'not_connected' }),
    setupStatus: async () => ({ needed: false, pending: [] }),
    ...over,
  };
}

afterEach(() => {
  resetRommForTest();
  session.phase = 'loading';
  session.setupReturn = null;
});

describe('phase reducers (decision 9)', () => {
  it('a card connect that needs setup opens Choose your systems, returning to Settings', () => {
    expect(phaseAfterConnect('settings', true)).toEqual({ phase: 'setup', setupReturn: 'settings' });
    expect(phaseAfterConnect('settings', false)).toBeNull();
  });
  it('a welcome connect waits for Continue', () => {
    expect(phaseAfterConnect('welcome', true)).toBeNull();
    expect(phaseAfterWelcome(true, true)).toEqual({ phase: 'setup', setupReturn: 'welcome' });
    expect(phaseAfterWelcome(true, false)).toEqual({ phase: 'shell', setupReturn: null });
    expect(phaseAfterWelcome(false, true)).toEqual({ phase: 'shell', setupReturn: null });
  });
});

describe('connectRomm', () => {
  it('from the card with setup needed: status connected, phase setup, setupReturn settings', async () => {
    configureRomm(fakeApi({ setupStatus: async () => ({ needed: true, pending: [] }) }));
    session.phase = 'shell';
    expect(await connectRomm('https://romm.home.lan', '', 'FAKE-TEST-TOKEN-not-real', true, 'settings')).toBeNull();
    expect(romm.status.connection).toBe('connected');
    expect(session.connected).toBe(true);
    expect(session.phase).toBe('setup');
    expect(session.setupReturn).toBe('settings');
  });

  it('from the welcome: the phase stays', async () => {
    configureRomm(fakeApi({ setupStatus: async () => ({ needed: true, pending: [] }) }));
    session.phase = 'welcome';
    expect(await connectRomm('https://romm.home.lan', '', 'x', true, 'welcome')).toBeNull();
    expect(session.phase).toBe('welcome');
  });

  it('a failed connect resolves to the error text and leaves the status', async () => {
    configureRomm(
      fakeApi({
        rommConnect: async () => {
          throw 'the server rejected the credentials';
        },
      }),
    );
    session.phase = 'shell';
    expect(await connectRomm('https://romm.home.lan', '', 'x', true, 'settings')).toBe('the server rejected the credentials');
    expect(romm.status.connection).toBe('not_connected');
    expect(session.phase).toBe('shell');
  });
});

describe('reconnect and disconnect', () => {
  it('take the status the backend answers with', async () => {
    configureRomm(fakeApi());
    await reconnectRomm();
    expect(romm.status.connection).toBe('connected');
    await disconnectRomm();
    expect(romm.status.connection).toBe('not_connected');
    expect(session.connected).toBe(false);
  });

  it('a failed reconnect pulls the status again', async () => {
    const pulled: RommStatus = rommStatus({ connection: 'unreachable', last_error: 'boom' });
    configureRomm(
      fakeApi({
        rommStatus: async () => pulled,
        rommReconnect: async () => {
          throw 'no stored session';
        },
      }),
    );
    expect(await reconnectRomm()).toBe('no stored session');
    expect(romm.status).toEqual(pulled);
  });
});
