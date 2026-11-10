import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry, InstalledGame } from '../api';
import type { DetailsSubject } from './subject';

type Fn = ReturnType<typeof vi.fn>;

function subject(over: Partial<DetailsSubject> = {}): DetailsSubject {
  return {
    romId: 7,
    name: 'Chrono Trigger',
    platformName: 'SNES',
    platformSlug: 'snes',
    coverSmall: null,
    coverLarge: null,
    screenshotUrls: [],
    fanartUrls: [],
    description: '',
    rating: '',
    genres: '',
    source: 'server',
    ...over,
  };
}

function row(over: Partial<InstalledGame> = {}): InstalledGame {
  return {
    title: 'Chrono Trigger',
    platform: 'SNES',
    platform_slug: 'snes',
    rom_id: 7,
    rom_file_name: 'ct.sfc',
    revision: '',
    installed_at: 0,
    last_played_at: 0,
    regions: '',
    languages: '',
    ...over,
  } as InstalledGame;
}

describe('DetailsStore', () => {
  let api: Record<string, Fn>;
  let run: Record<string, Fn>;
  let state: {
    rows: InstalledGame[];
    entries: DownloadEntry[];
    session: { id: number } | undefined;
    connected: boolean;
  };
  let removeFromDrive: Fn;
  let openBackup: Fn;

  beforeEach(() => {
    vi.resetModules();
    state = { rows: [], entries: [], session: undefined, connected: true };
    api = {
      getRomDetail: vi.fn(async () => ({ platform_id: 3, related: [], files: [], fs_name: '', revision: '' })),
      getLaunchDefaults: vi.fn(async () => ({ default_emulators: {} })),
      listGames: vi.fn(async () => [{ name: 'Secret of Mana' }]),
      installBlockReason: vi.fn(async () => 'No emulator for SNES'),
      contentAvailability: vi.fn(async () => ({ update: true, dlc: false })),
      cloudPanelInfo: vi.fn(async (_g: unknown, kind: string) => ({ kind })),
    };
    run = {
      installServerGame: vi.fn(async () => ({ kind: 'ok' })),
      uninstallGame: vi.fn(async () => ({ kind: 'ok' })),
      updateGame: vi.fn(async () => ({ kind: 'ok' })),
      cancelInstall: vi.fn(async () => ({ kind: 'ok' })),
      installContent: vi.fn(async () => ({ kind: 'ok' })),
      playInstalled: vi.fn(async () => ({ kind: 'ok' })),
      stopGame: vi.fn(async () => ({ kind: 'ok' })),
      linkGame: vi.fn(async () => ({ kind: 'ok' })),
      locateGame: vi.fn(async () => ({ kind: 'ok' })),
      removeFromLibrary: vi.fn(async () => ({ kind: 'ok' })),
      installFromDrive: vi.fn(async () => ({ kind: 'ok' })),
    };
    removeFromDrive = vi.fn(async () => {});
    openBackup = vi.fn();
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../games/run', () => run);
    vi.doMock('../backups/install', () => ({ removeFromDrive }));
    vi.doMock('../backups/dialogState.svelte', () => ({ openBackup }));
    vi.doMock('../stores/lastViewed.svelte', () => ({ noteViewed: vi.fn() }));
    vi.doMock('../stores/installed.svelte', () => ({
      installed: {
        get list() {
          return state.rows;
        },
      },
      isInstalled: (summary: { name: string }) => state.rows.some((r) => r.title === summary.name),
      matchesInstalled: (r: InstalledGame, summary: { name: string }) => r.title === summary.name,
    }));
    vi.doMock('../stores/downloads.svelte', () => ({
      downloads: {
        get entries() {
          return state.entries;
        },
      },
    }));
    vi.doMock('../stores/sessions.svelte', () => ({ sessions: { sessionFor: () => state.session } }));
    vi.doMock('../stores/updates.svelte', () => ({ updates: { labelFor: () => null } }));
    vi.doMock('../stores/drives.svelte', () => ({ drives: { holdingRom: () => [] } }));
    vi.doMock('../stores/session.svelte', () => ({
      session: {
        get connected() {
          return state.connected;
        },
        serverUrl: 'https://romm.test',
      },
    }));
  });

  afterEach(() => {
    for (const m of [
      '../api',
      '../games/run',
      '../backups/install',
      '../backups/dialogState.svelte',
      '../stores/lastViewed.svelte',
      '../stores/installed.svelte',
      '../stores/downloads.svelte',
      '../stores/sessions.svelte',
      '../stores/updates.svelte',
      '../stores/drives.svelte',
      '../stores/session.svelte',
    ])
      vi.doUnmock(m);
  });

  async function store(s: DetailsSubject = subject()) {
    const { DetailsStore } = await import('./detailsStore.svelte');
    return new DetailsStore(() => s);
  }

  describe('primary', () => {
    it('is install for a server game not installed', async () => {
      expect((await store()).primary).toBe('install');
    });

    it('is install-drive for a drive game', async () => {
      const drive = { key: '/media/kit', name: 'Couch kit', label: 'KIT', server: 'romm.test' };
      expect((await store(subject({ drive, source: 'drive' }))).primary).toBe('install-drive');
    });

    it('is play once installed, stop while running, cancel during an install', async () => {
      state.rows = [row()];
      expect((await store()).primary).toBe('play');
      state.session = { id: 4 };
      expect((await store()).primary).toBe('stop');
      state.entries = [{ rom_id: 7, status: 'downloading' } as DownloadEntry];
      expect((await store()).primary).toBe('cancel');
    });

    it('is link for an installed row with no server id, no-id otherwise', async () => {
      state.rows = [row({ rom_id: null })];
      expect((await store(subject({ romId: null }))).primary).toBe('link');
      state.rows = [];
      expect((await store(subject({ romId: null }))).primary).toBe('no-id');
    });
  });

  it('reads a missing row as blocked and as the remove target', async () => {
    state.rows = [row({ missing: true } as Partial<InstalledGame>)];
    const d = await store();
    expect(d.missing.showMissing).toBe(true);
    expect(d.missing.playBlocked).not.toBe('');
    expect(d.removeTarget?.title).toBe('Chrono Trigger');
  });

  describe('loaders', () => {
    it('fetches the detail only when connected', async () => {
      state.connected = false;
      const off = await store();
      off.loadDetail();
      expect(api.getRomDetail).not.toHaveBeenCalled();
      state.connected = true;
      const d = await store();
      d.loadDetail();
      await vi.waitFor(() => expect(d.detail).not.toBeNull());
      expect(api.getRomDetail).toHaveBeenCalledWith(7);
    });

    it('asks for the install-block reason only for a game not installed', async () => {
      const d = await store();
      d.refreshInstallBlocked();
      await vi.waitFor(() => expect(d.installBlocked).toBe('No emulator for SNES'));
      state.rows = [row()];
      const inst = await store();
      inst.installBlocked = 'stale';
      inst.refreshInstallBlocked();
      expect(inst.installBlocked).toBe('');
    });

    it('filters Related only when the detail lists related games', async () => {
      const d = await store();
      d.detail = { platform_id: 3, related: [] } as never;
      expect(d.loadServerTitles()).toBeUndefined();
      d.detail = { platform_id: 3, related: [{ id: 1 }] } as never;
      const stop = d.loadServerTitles();
      expect(stop).toBeTypeOf('function');
      await vi.waitFor(() => expect(d.serverTitles).toEqual(['Secret of Mana']));
    });

    it('drops a Related answer that arrives after the stop', async () => {
      const d = await store();
      d.detail = { platform_id: 3, related: [{ id: 1 }] } as never;
      d.loadServerTitles()?.();
      await Promise.resolve();
      await Promise.resolve();
      expect(d.serverTitles).toEqual([]);
    });

    it('fetches content availability for an installed content game, again after an install ends', async () => {
      state.rows = [row({ title: 'Bloodborne', platform: 'PS4' })];
      const s = subject({ name: 'Bloodborne', platformName: 'PS4', platformSlug: 'ps4' });
      const d = await store(s);
      d.refreshContentAvailability();
      await vi.waitFor(() => expect(d.contentAvailability).not.toBeNull());
      expect(api.contentAvailability).toHaveBeenCalledTimes(1);
      d.refreshContentAvailability(); // nothing changed: no second fetch
      expect(api.contentAvailability).toHaveBeenCalledTimes(1);
      d.wasLive = true; // an install for it just ended
      d.refreshContentAvailability();
      expect(api.contentAvailability).toHaveBeenCalledTimes(2);
    });

    it('loads both cloud panels for a game with a server id', async () => {
      const d = await store();
      d.loadCloudPanelInfo();
      await vi.waitFor(() => expect(d.statePanelInfo).not.toBeNull());
      expect(d.savePanelInfo).toEqual({ kind: 'save' });
    });
  });

  describe('actions', () => {
    it('install answers the outcome and clears pending', async () => {
      run.installServerGame.mockResolvedValueOnce({ kind: 'needs-library-folder', reason: 'no folder' });
      const d = await store();
      const outcome = d.install();
      expect(d.pendingAction).toBe('install');
      expect(await outcome).toEqual({ kind: 'needs-library-folder', reason: 'no folder' });
      expect(d.pendingAction).toBeNull();
    });

    it('uninstall reports a refusal in error', async () => {
      run.uninstallGame.mockResolvedValueOnce({ kind: 'refused', reason: 'busy' });
      const d = await store();
      expect(await d.uninstall()).toBe(false);
      expect(d.error).toBe('busy');
    });

    it('play shows a reason inline only with the umu offer', async () => {
      run.playInstalled.mockResolvedValueOnce({ kind: 'refused', reason: 'no emulator' });
      const d = await store();
      await d.play();
      expect(d.error).toBeNull();
      run.playInstalled.mockResolvedValueOnce({ kind: 'needs-umu', reason: 'umu-run missing' });
      await d.play();
      expect(d.error).toBe('umu-run missing');
      expect(d.umuOfferFor).toBe('umu-run missing');
    });

    it('stop needs a running session', async () => {
      const d = await store();
      await d.stop();
      expect(run.stopGame).not.toHaveBeenCalled();
      state.session = { id: 9 };
      await (await store()).stop();
      expect(run.stopGame).toHaveBeenCalledWith(9);
    });

    it('locate puts a refusal in locateError', async () => {
      state.rows = [row({ missing: true } as Partial<InstalledGame>)];
      run.locateGame.mockResolvedValueOnce({ kind: 'refused', reason: 'not there' });
      const d = await store();
      expect(await d.locate('  ')).toBe(false);
      expect(await d.locate('/games/ct')).toBe(false);
      expect(d.locateError).toBe('not there');
      expect(await d.locate('/games/ct')).toBe(true);
      expect(d.locateError).toBeNull();
    });

    it('linkTo and removeFromLibrary throw a refusal', async () => {
      state.rows = [row({ rom_id: null })];
      run.linkGame.mockResolvedValueOnce({ kind: 'refused', reason: 'taken' });
      run.removeFromLibrary.mockResolvedValueOnce({ kind: 'refused', reason: 'locked' });
      const d = await store(subject({ romId: null }));
      await expect(d.linkTo({ id: 3 } as never)).rejects.toThrow('taken');
      await expect(d.removeFromLibrary()).rejects.toThrow('locked');
      expect(await d.removeFromLibrary()).toBe(true);
    });

    it('drive install and remove act on the subject drive', async () => {
      const drive = { key: '/media/kit', name: 'Couch kit', label: 'KIT', server: 'romm.test' };
      const d = await store(subject({ drive, source: 'drive' }));
      await d.installFromDrive();
      expect(run.installFromDrive).toHaveBeenCalledWith('/media/kit', {
        rom_id: 7,
        name: 'Chrono Trigger',
        server: 'romm.test',
      });
      removeFromDrive.mockRejectedValueOnce(new Error('read-only'));
      expect(await d.removeFromDrive()).toBe(false);
      expect(d.error).toBe('read-only');
      expect(await d.removeFromDrive()).toBe(true);
    });

    it('backUp opens the dialog for the game while connected', async () => {
      const onLeave = vi.fn();
      (await store()).backUp(onLeave);
      expect(openBackup).toHaveBeenCalledWith(expect.objectContaining({ onLeave }));
      state.connected = false;
      openBackup.mockClear();
      (await store()).backUp(onLeave);
      expect(openBackup).not.toHaveBeenCalled();
    });
  });
});
