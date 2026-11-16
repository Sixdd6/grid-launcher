import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

// Legacy TV Connect helpers (Step 6 replaces TV Connect, Step 9 deletes them).
describe('connectFlow', () => {
  let api: Record<string, Fn>;
  let useOffline: Fn;

  beforeEach(() => {
    vi.resetModules();
    api = {
      getLibraryPath: vi.fn(async () => ''),
      suggestedLibraryPath: vi.fn(async () => ' /home/me/Games '),
      setLibraryPath: vi.fn(async () => {}),
    };
    useOffline = vi.fn(async () => {});
    vi.doMock('./api', () => ({ api }));
    vi.doMock('./stores/session.svelte', () => ({ useOffline }));
  });

  afterEach(() => {
    vi.doUnmock('./api');
    vi.doUnmock('./stores/session.svelte');
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
});
