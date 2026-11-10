import { beforeEach, describe, expect, it, vi } from 'vitest';

// Menu › Reconnect (item 364, README § Consultant decisions).
const session = { offline: false, connected: false, busy: false, serverUrl: 'https://romm.home.lan', phase: 'shell', lastError: null as string | null };
const retry = vi.fn(async () => {});
const connectToServer = vi.fn();
const pushToast = vi.fn();

vi.mock('../stores/session.svelte', () => ({ session, retry, connectToServer }));
vi.mock('../stores/toasts.svelte', () => ({ pushToast }));

const { reconnect } = await import('./reconnect');

describe('reconnect', () => {
  beforeEach(() => {
    Object.assign(session, { offline: false, connected: false, busy: false, phase: 'shell', lastError: null });
    retry.mockReset();
    connectToServer.mockReset();
    pushToast.mockReset();
  });

  it('does nothing while a probe is already running', async () => {
    session.busy = true;
    await reconnect();
    expect(retry).not.toHaveBeenCalled();
    expect(pushToast).not.toHaveBeenCalled();
  });

  it('opens Connect when used without a server', async () => {
    session.offline = true;
    await reconnect();
    expect(connectToServer).toHaveBeenCalledOnce();
    expect(retry).not.toHaveBeenCalled();
  });

  it('says it connected', async () => {
    retry.mockImplementation(async () => {
      session.connected = true;
    });
    await reconnect();
    expect(pushToast.mock.calls.map((c) => c[0])).toEqual(['Reconnecting to https://romm.home.lan…', 'Connected to https://romm.home.lan']);
  });

  it("shows Can't reach with the reason verbatim, or no second line", async () => {
    retry.mockImplementation(async () => {
      session.lastError = 'connection refused';
    });
    await reconnect();
    expect(pushToast).toHaveBeenLastCalledWith("Can't reach https://romm.home.lan", 'error', undefined, { sub: 'connection refused' });
    session.lastError = null;
    retry.mockImplementation(async () => {});
    await reconnect();
    expect(pushToast).toHaveBeenLastCalledWith("Can't reach https://romm.home.lan", 'error', undefined, { sub: undefined });
  });

  it('shows no card when a rejected credential opened Connect', async () => {
    retry.mockImplementation(async () => {
      session.phase = 'none';
    });
    await reconnect();
    expect(pushToast.mock.calls.map((c) => c[0])).toEqual(['Reconnecting to https://romm.home.lan…']);
  });
});
