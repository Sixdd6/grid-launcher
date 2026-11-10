import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

describe('raActions', () => {
  let api: Record<string, Fn>;
  let toasts: [string, string | undefined][];

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    api = {
      getRetroachievementsStatus: vi.fn(async () => ({ username: 'player', token_present: true })),
      setRetroachievementsCredentials: vi.fn(async () => []),
      retroachievementsLogin: vi.fn(async () => ({ username: 'player', fan_out: [] })),
      clearRetroachievementsCredentials: vi.fn(async () => {}),
      syncRaUsernameToRomm: vi.fn(async () => ({ kind: 'synced' })),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../stores/toasts.svelte', () => ({
      pushToast: (text: string, level?: string) => toasts.push([text, level]),
    }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('../stores/toasts.svelte');
  });

  it('saving a token answers the summary and syncs the username', async () => {
    const { saveRaToken } = await import('./raActions');
    expect(typeof (await saveRaToken('player', 'FAKE-RA-TOKEN-not-real'))).toBe('string');
    expect(api.setRetroachievementsCredentials).toHaveBeenCalledWith('player', 'FAKE-RA-TOKEN-not-real');
    expect(api.syncRaUsernameToRomm).toHaveBeenCalledTimes(1);
  });

  it('a refused token save throws and syncs nothing', async () => {
    api.setRetroachievementsCredentials.mockRejectedValueOnce(new Error('bad token'));
    const { saveRaToken } = await import('./raActions');
    await expect(saveRaToken('player', 'FAKE-RA-TOKEN-not-real')).rejects.toThrow('bad token');
    expect(api.syncRaUsernameToRomm).not.toHaveBeenCalled();
  });

  it('a login with a blank field sends nothing', async () => {
    const { raLogin } = await import('./raActions');
    expect(await raLogin('player', '')).toEqual({ kind: 'missing' });
    expect(api.retroachievementsLogin).not.toHaveBeenCalled();
    expect(toasts[0][1]).toBe('error');
  });

  it('a login answers the summary with its card, or the failure with its card', async () => {
    const { raLogin } = await import('./raActions');
    expect((await raLogin('player', 'pw')).kind).toBe('ok');
    expect(toasts).toHaveLength(1);
    api.retroachievementsLogin.mockRejectedValueOnce(new Error('wrong password'));
    expect(await raLogin('player', 'pw')).toEqual({ kind: 'failed', error: 'wrong password' });
    expect(toasts.at(-1)?.[1]).toBe('error');
  });

  it('a failed RomM sync is a card, never an error', async () => {
    api.syncRaUsernameToRomm.mockRejectedValueOnce(new Error('offline'));
    const { syncUsernameToRomm } = await import('./raActions');
    syncUsernameToRomm();
    await vi.waitFor(() => expect(toasts.length).toBeGreaterThan(0));
    expect(toasts[0][1]).toBe('error');
  });

  it('clear sends its card; status reads the account', async () => {
    const { clearRaCredentials, fetchRaStatus } = await import('./raActions');
    await clearRaCredentials();
    expect(toasts).toHaveLength(1);
    expect((await fetchRaStatus()).username).toBe('player');
  });
});
