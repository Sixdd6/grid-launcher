import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { GameSummary, Platform } from '../api';

type Handler = (e: { payload: unknown }) => void;

function platform(id: number, display_name: string, slug = 'snes'): Platform {
  return { id, name: display_name, slug, rom_count: 3, custom_name: null, display_name };
}

function game(id: number, platform_id: number): GameSummary {
  return { id, name: `Game ${id}`, platform_id, platform_display_name: 'Elsewhere' } as GameSummary;
}

/** A promise the test settles by hand. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('Catalog', () => {
  let api: Record<string, ReturnType<typeof vi.fn>>;
  let handlers: Record<string, Handler>;
  let unlistened: string[];

  beforeEach(() => {
    vi.resetModules();
    handlers = {};
    unlistened = [];
    api = {
      listPlatforms: vi.fn(async () => [platform(1, 'SNES'), platform(2, 'Windows', 'win')]),
      listGames: vi.fn(async (id: number) => [game(10 + id, id)]),
      getLaunchDefaults: vi.fn(async () => ({ default_emulators: { SNES: 'RetroArch', GBA: '' } })),
      platformFirmwareStatus: vi.fn(async () => ({ file_count: 1 })),
      getLibraryPath: vi.fn(async () => ''),
      setLibraryPath: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({
      api,
      EMULATOR_INSTALLED_EVENT: 'emulator-installed',
      FIRMWARE_PASS_FINISHED_EVENT: 'firmware-pass-finished',
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

  async function make() {
    const { Catalog } = await import('./catalog.svelte');
    return new Catalog();
  }

  it('loads the platforms and names a game by its platform', async () => {
    const c = await make();
    await c.loadPlatforms();
    expect(c.platforms.map((p) => p.id)).toEqual([1, 2]);
    expect(c.labelFor(game(5, 1))).toBe('SNES');
    // A game of a platform not in the list keeps its own display name.
    expect(c.labelFor(game(5, 9))).toBe('Elsewhere');
  });

  it('selects a platform: name at once, games when they answer', async () => {
    const c = await make();
    await c.loadPlatforms();
    const answer = deferred<GameSummary[]>();
    api.listGames.mockReturnValueOnce(answer.promise);
    const done = c.selectPlatform(1);
    expect(c.activePlatform).toBe(1);
    expect(c.activePlatformName).toBe('SNES');
    expect(c.gamesLoading).toBe(true);
    answer.resolve([game(11, 1)]);
    expect(await done).toBe(true);
    expect(c.games.map((g) => g.id)).toEqual([11]);
    expect(c.gamesLoading).toBe(false);
    expect(c.chipsShown).toBe(true);
  });

  it('drops an answer a newer selection superseded', async () => {
    const c = await make();
    await c.loadPlatforms();
    const slow = deferred<GameSummary[]>();
    api.listGames.mockReturnValueOnce(slow.promise);
    const first = c.selectPlatform(1);
    expect(await c.selectPlatform(2)).toBe(true);
    slow.resolve([game(11, 1)]);
    expect(await first).toBe(false);
    expect(c.activePlatform).toBe(2);
    expect(c.games.map((g) => g.id)).toEqual([12]);
    // Windows shows no emulator or firmware chip.
    expect(c.chipsShown).toBe(false);
  });

  it('empties the list when the games cannot be read', async () => {
    const c = await make();
    await c.selectPlatform(1);
    api.listGames.mockRejectedValueOnce(new Error('offline'));
    expect(await c.selectPlatform(2)).toBe(false);
    expect(c.games).toEqual([]);
    expect(c.gamesLoading).toBe(false);
  });

  it('reads the launch defaults into chips and cloud badges, and survives a refusal', async () => {
    const c = await make();
    await c.loadLaunchDefaults();
    expect(c.defaultEmulators).toEqual({ SNES: 'RetroArch', GBA: '' });
    expect([...c.cloudPlatforms]).toEqual(['snes']);
    api.getLaunchDefaults.mockRejectedValueOnce(new Error('no config'));
    await c.loadLaunchDefaults();
    expect(c.defaultEmulators).toEqual({ SNES: 'RetroArch', GBA: '' });
  });

  it('keeps a firmware answer only for the platform still active', async () => {
    const c = await make();
    await c.loadPlatforms();
    await c.selectPlatform(1);
    await c.refreshFirmwareStatus(1, 'SNES');
    expect(c.firmware).toEqual({ file_count: 1 });
    api.platformFirmwareStatus.mockRejectedValueOnce(new Error('offline'));
    await c.refreshFirmwareStatus(1, 'SNES');
    expect(c.firmware).toBe('unavailable');
    c.firmware = null;
    await c.selectPlatform(2);
    await c.refreshFirmwareStatus(1, 'SNES');
    expect(c.firmware).toBeNull();
  });

  it('knows when no library folder is set, and clears it on save', async () => {
    const c = await make();
    await c.checkLibraryPath();
    expect(c.libraryPathUnset).toBe(true);
    api.getLibraryPath.mockRejectedValueOnce(new Error('unreadable'));
    await c.checkLibraryPath();
    expect(c.libraryPathUnset).toBe(true);
    await c.saveLibraryPath('  /games  ');
    expect(api.setLibraryPath).toHaveBeenCalledWith('/games');
    expect(c.libraryPathUnset).toBe(false);
  });

  it('passes a refused library folder to the screen', async () => {
    const c = await make();
    c.libraryPathUnset = true;
    api.setLibraryPath.mockRejectedValueOnce(new Error('Not writable'));
    await expect(c.saveLibraryPath('/ro')).rejects.toThrow('Not writable');
    expect(c.libraryPathUnset).toBe(true);
  });

  it('follows firmware and defaults events while watched', async () => {
    const c = await make();
    await c.loadPlatforms();
    await c.selectPlatform(1);
    const stop = c.watch();
    await Promise.resolve();
    api.platformFirmwareStatus.mockClear();
    api.getLaunchDefaults.mockClear();

    handlers['firmware-pass-finished']({ payload: { platform_id: 2, ok: true, failed: 0 } });
    expect(api.platformFirmwareStatus).not.toHaveBeenCalled();
    handlers['firmware-pass-finished']({ payload: { platform_id: 1, ok: true, failed: 0 } });
    expect(api.platformFirmwareStatus).toHaveBeenCalledWith(1, 'SNES');

    handlers['emulator-installed']({ payload: {} });
    handlers['setup-defaults']({ payload: [] });
    expect(api.getLaunchDefaults).toHaveBeenCalledTimes(2);

    stop();
    expect(unlistened.sort()).toEqual(['emulator-installed', 'firmware-pass-finished', 'setup-defaults']);
  });
});
