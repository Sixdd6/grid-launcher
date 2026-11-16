import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

describe('welcomeFlow', () => {
  let api: Record<string, Fn>;

  beforeEach(() => {
    vi.resetModules();
    api = {
      getLibraryPath: vi.fn(async () => ''),
      suggestedLibraryPath: vi.fn(async () => ' /home/me/Games/GRID '),
      setLibraryPath: vi.fn(async () => {}),
    };
    vi.doMock('./api', () => ({ api }));
  });

  afterEach(() => {
    vi.doUnmock('./api');
  });

  it('starts the field on the suggestion, on the stored folder when there is one, blank when unreadable', async () => {
    const f = await import('./welcomeFlow');
    expect(await f.loadLibraryFolder()).toEqual({ stored: '', initial: '/home/me/Games/GRID' });
    api.getLibraryPath.mockResolvedValueOnce('/data/RomM');
    expect(await f.loadLibraryFolder()).toEqual({ stored: '/data/RomM', initial: '/data/RomM' });
    api.getLibraryPath.mockRejectedValueOnce(new Error('no config'));
    expect(await f.loadLibraryFolder()).toEqual({ stored: '', initial: '' });
  });

  it('stores a changed folder, skips an unchanged one, answers the refusal line', async () => {
    const f = await import('./welcomeFlow');
    expect(await f.saveLibraryFolder('/data/RomM', '/data/RomM')).toBeNull();
    expect(api.setLibraryPath).not.toHaveBeenCalled();
    expect(await f.saveLibraryFolder(' /games ', '')).toBeNull();
    expect(api.setLibraryPath).toHaveBeenCalledWith('/games');
    api.setLibraryPath.mockRejectedValueOnce('not writable');
    expect(await f.saveLibraryFolder('/ro', '')).toBe("That folder can't be used: not writable");
  });
});
