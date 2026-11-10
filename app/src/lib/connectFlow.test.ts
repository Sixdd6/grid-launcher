import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

describe('connectFlow', () => {
  let api: Record<string, Fn>;
  let connect: Fn;
  let useOffline: Fn;

  beforeEach(() => {
    vi.resetModules();
    api = {
      getLibraryPath: vi.fn(async () => ''),
      suggestedLibraryPath: vi.fn(async () => ' /home/me/Games '),
      setLibraryPath: vi.fn(async () => {}),
    };
    connect = vi.fn(async () => {});
    useOffline = vi.fn(async () => {});
    vi.doMock('./api', () => ({ api }));
    vi.doMock('./stores/session.svelte', () => ({ connect, useOffline }));
  });

  afterEach(() => {
    vi.doUnmock('./api');
    vi.doUnmock('./stores/session.svelte');
  });

  it('starts the folder field on the suggestion, hides it when one is stored, asks blank when unreadable', async () => {
    const f = await import('./connectFlow');
    expect(await f.loadLibraryField()).toBe('/home/me/Games');
    api.getLibraryPath.mockResolvedValueOnce('/stored');
    expect(await f.loadLibraryField()).toBeNull();
    api.getLibraryPath.mockRejectedValueOnce(new Error('no config'));
    expect(await f.loadLibraryField()).toBe('');
  });

  it('stores a typed folder, or answers the refusal line', async () => {
    const f = await import('./connectFlow');
    expect(await f.saveConnectLibraryPath(null)).toBeNull();
    expect(api.setLibraryPath).not.toHaveBeenCalled();
    expect(await f.saveConnectLibraryPath(' /games ')).toBeNull();
    expect(api.setLibraryPath).toHaveBeenCalledWith('/games');
    api.setLibraryPath.mockRejectedValueOnce('not writable');
    expect(await f.saveConnectLibraryPath('/ro')).toBe("That folder can't be used: not writable");
  });

  it('goes offline only once the folder is stored', async () => {
    const f = await import('./connectFlow');
    api.setLibraryPath.mockRejectedValueOnce('not writable');
    expect(await f.useWithoutServer('/ro')).toBe("That folder can't be used: not writable");
    expect(useOffline).not.toHaveBeenCalled();
    expect(await f.useWithoutServer('/games')).toBeNull();
    expect(useOffline).toHaveBeenCalledTimes(1);
  });

  it('never sends a username with a token', async () => {
    const f = await import('./connectFlow');
    f.startConnect('https://romm.test', 'stale', 'FAKE-TEST-TOKEN-not-real', true);
    expect(connect).toHaveBeenLastCalledWith('https://romm.test', '', 'FAKE-TEST-TOKEN-not-real', true);
    f.startConnect('https://romm.test', 'me', 'FAKE-TEST-TOKEN-not-real', false);
    expect(connect).toHaveBeenLastCalledWith('https://romm.test', 'me', 'FAKE-TEST-TOKEN-not-real', false);
  });
});
