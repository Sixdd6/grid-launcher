import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlatformSetup, SetupOption, SystemPlatform } from '../api';

type Fn = ReturnType<typeof vi.fn>;
type Handler = (e: { payload: unknown }) => void;

const opt = (source_id: string, name: string): SetupOption => ({
  source_id,
  name,
  kind: 'standalone',
  installed: false,
  server: null,
  needs_core: false,
});

function setup(label: string, name: string, options: SetupOption[]): PlatformSetup {
  return { label, name, slug: label.toLowerCase(), rom_count: 4, options, recommended: options[0]?.source_id ?? null, core: null };
}

/** A system on RomM, keyed by the setup row's slug (its label, lower case). */
const platform = (id: number, label: string, slug = label.toLowerCase()): SystemPlatform => ({
  slug,
  label,
  name: label,
  aliases: [],
  origin: 'both',
  server_id: id,
  server_rom_count: 4,
  installed_count: 0,
});

const ps2 = setup('PlayStation 2', 'Sony PlayStation 2', [opt('PCSX2/pcsx2', 'PCSX2 (Playstation 2)')]);
const dc = setup('Dreamcast', 'Sega Dreamcast', [opt('redream/redream', 'Redream (Sega Dreamcast)')]);

describe('SystemsStore', () => {
  let api: Record<string, Fn>;
  let toasts: [string, string | undefined][];
  let handlers: Record<string, Handler>;
  let hasUpdate: (name: string) => boolean;

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    handlers = {};
    hasUpdate = () => false;
    api = {
      listEmulators: vi.fn(async () => [{ name: 'RetroArch', source_id: 'libretro/RetroArch' }]),
      emulatorCompanions: vi.fn(async () => []),
      getLaunchDefaults: vi.fn(async () => ({ default_emulators: {}, retroarch_cores: {}, launch_args: '' })),
      systemPlatforms: vi.fn(async () => [
        platform(1, 'PlayStation 2'),
        platform(2, 'Dreamcast'),
        platform(3, 'Windows', 'win'),
      ]),
      firmwareExpected: vi.fn(async () => ({ files: [] })),
      setupPlatformOptions: vi.fn(async () => [ps2, dc]),
      setupStatus: vi.fn(async () => ({ pending: [] })),
      installForSystems: vi.fn(async () => ({ errors: [] })),
      setDefaultEmulator: vi.fn(async () => {}),
      setRetroarchCore: vi.fn(async () => {}),
      launchEmulator: vi.fn(async () => ''),
      updateEmulator: vi.fn(async () => {}),
      installEmulator: vi.fn(async () => {}),
      showEmulatorFolder: vi.fn(async () => {}),
      deleteEmulator: vi.fn(async () => ({ skipped: [] })),
      platformFirmwareStatus: vi.fn(async () => ({ file_count: 1 })),
      installFirmwareForPlatform: vi.fn(async () => {}),
      installPs3Firmware: vi.fn(async () => true),
      rpcs3FirmwareStatus: vi.fn(async () => ({ installed: true })),
      emulatorFacts: vi.fn(async () => ({})),
      compatibleEmulators: vi.fn(async () => ({ 'Sega Dreamcast': ['Redream (Sega Dreamcast)'] })),
      retroarchCoreOptions: vi.fn(async () => ({})),
      listProfiles: vi.fn(async () => []),
    };
    vi.doMock('../api', () => ({
      api,
      FIRMWARE_PASS_FINISHED_EVENT: 'firmware-pass-finished',
      SETUP_DEFAULTS_EVENT: 'setup-defaults',
    }));
    vi.doMock('@tauri-apps/api/event', () => ({
      listen: (name: string, handler: Handler) => {
        handlers[name] = handler;
        return Promise.resolve(() => delete handlers[name]);
      },
    }));
    vi.doMock('../stores/downloads.svelte', () => ({ downloads: { entries: [] } }));
    vi.doMock('../stores/emulatorUpdates.svelte', () => ({
      emulatorUpdates: { running: false, hasUpdate: (n: string) => hasUpdate(n), check: () => null },
      checkOncePerSession: vi.fn(async () => {}),
      recheck: vi.fn(async () => {}),
    }));
    vi.doMock('../stores/toasts.svelte', () => ({
      pushToast: (text: string, level?: string) => toasts.push([text, level]),
    }));
    vi.doMock('../notifications/notifier', () => ({ expectQueued: vi.fn() }));
    vi.doMock('../stores/romm.svelte', () => ({ romm: { status: { connection: 'not_connected' } } }));
  });

  afterEach(() => {
    for (const m of [
      '../api',
      '../stores/romm.svelte',
      '@tauri-apps/api/event',
      '../stores/downloads.svelte',
      '../stores/emulatorUpdates.svelte',
      '../stores/toasts.svelte',
      '../notifications/notifier',
    ])
      vi.doUnmock(m);
  });

  async function store(active = () => true) {
    const { SystemsStore } = await import('./systemsStore.svelte');
    return new SystemsStore(active);
  }

  async function loadedStore() {
    const s = await store();
    await s.refreshAll();
    s.compatible = { 'Sega Dreamcast': ['Redream (Sega Dreamcast)'] };
    return s;
  }

  it('refreshAll loads the rows, missing ones first', async () => {
    const s = await loadedStore();
    expect(s.loaded).toBe(true);
    expect(s.rows.map((r) => [r.label, r.emulator])).toEqual([
      ['PlayStation 2', ''],
      ['Dreamcast', 'Redream (Sega Dreamcast)'],
    ]);
    expect(s.missing).toEqual([{ platform: 'PlayStation 2', source_id: 'PCSX2/pcsx2' }]);
    expect(s.windowsGames).toBe(4);
  });

  it('a refusal while loading is the page error', async () => {
    api.systemPlatforms.mockRejectedValueOnce(new Error('offline'));
    const s = await store();
    await s.refreshAll();
    expect(s.loadError).toBe('offline');
  });

  it('reads the files a system needs and builds its firmware block without RomM', async () => {
    const s = await loadedStore();
    api.firmwareExpected.mockResolvedValueOnce({
      files: [{ name: 'dc_boot.bin', size: 4, md5: 'a'.repeat(32), required: true, installed: false, verified_by: 'grid', region: '' }],
    });
    const dcRow = s.rows.find((r) => r.label === 'Dreamcast')!;
    s.refreshExpected(dcRow);
    await vi.waitFor(() => expect(s.expected['dreamcast|Redream (Sega Dreamcast)']).toHaveLength(1));
    expect(api.firmwareExpected).toHaveBeenCalledWith('dreamcast', 'Redream (Sega Dreamcast)');
    const view = s.firmwareView(dcRow);
    expect(view.romm).toMatchObject({ disabled: true, why: 'needs-romm' });
    expect(view.locate).toMatchObject({ disabled: false, primary: true });
    expect(s.rowStatus(dcRow).status).toBe('firmware');
  });

  it('Locate files sends the picks and re-reads the expected files', async () => {
    const s = await loadedStore();
    api.locateFirmwareFiles = vi.fn(async () => ({ installed: [{ file: 'a.bin', as_name: 'dc_boot.bin' }], unverified: [], refused: [], unmatched: 0 }));
    const dcRow = s.rows.find((r) => r.label === 'Dreamcast')!;
    const answer = await s.locateFiles(dcRow, ['/picks/a.bin']);
    expect(api.locateFirmwareFiles).toHaveBeenCalledWith('dreamcast', 'Redream (Sega Dreamcast)', ['/picks/a.bin']);
    expect(answer.installed).toHaveLength(1);
    expect(api.firmwareExpected).toHaveBeenCalled();
  });

  it('a PUP RPCS3 refuses comes back refused with its reason', async () => {
    const s = await loadedStore();
    api.locatePs3Firmware = vi.fn(async () => {
      throw new Error("scph39001.bin isn't a PlayStation 3 system update. Choose PS3UPDAT.PUP.");
    });
    const answer = await s.locatePup(s.rows[0], '/x/scph39001.bin');
    expect(answer.refused).toEqual([
      { file: 'scph39001.bin', reason: "scph39001.bin isn't a PlayStation 3 system update. Choose PS3UPDAT.PUP." },
    ]);
  });

  it('a system with no games and no emulator is Not set up', async () => {
    api.systemPlatforms.mockResolvedValueOnce([{ ...platform(0, 'PlayStation 2'), origin: 'catalog', server_id: null, server_rom_count: null }]);
    const s = await loadedStore();
    const row = s.rows.find((r) => r.label === 'PlayStation 2')!;
    expect(s.rowStatus(row).status).toBe('unset');
    expect(s.groups.more.map((r) => r.label)).toEqual(['PlayStation 2']);
    expect(s.missing).toEqual([]);
  });

  it('Install recommended counts its emulators while it runs and ignores a second press', async () => {
    const s = await loadedStore();
    let release!: (v: { errors: string[] }) => void;
    api.installForSystems.mockImplementationOnce(() => new Promise((res) => (release = res)));
    const run = s.installAllRecommended();
    expect(s.recommendedInstalling).toBe(1);
    expect(s.recommendedInstallingLabel).toBe('Installing 1 emulator…');
    expect(s.busyRows).toEqual({ 'PlayStation 2': true });
    await s.installAllRecommended();
    expect(api.installForSystems).toHaveBeenCalledTimes(1);
    release({ errors: ['RetroArch: no space'] });
    await run;
    expect(s.recommendedInstalling).toBe(0);
    expect(s.recommendedInstallingLabel).toBe('');
    expect(s.busyRows).toEqual({});
    expect(toasts).toContainEqual(['RetroArch: no space', 'error']);
  });

  it('a use pick sets the default and opens the row; None does not open it', async () => {
    const s = await loadedStore();
    const row = s.rows[0];
    expect(await s.pick(row, 'use:PCSX2')).toMatchObject({ opened: true });
    expect(api.setDefaultEmulator).toHaveBeenCalledWith('PlayStation 2', 'PCSX2');
    expect((await s.pick(row, 'none')).opened).toBe(false);
  });

  it('an install pick installs and shows the real emulator', async () => {
    const s = await loadedStore();
    const result = await s.pick(s.rows[0], 'install:PCSX2/pcsx2');
    expect(api.installForSystems).toHaveBeenCalledWith([{ platform: 'PlayStation 2', source_id: 'PCSX2/pcsx2' }]);
    expect(result.opened).toBe(false);
    expect(result.selected).not.toBe('install:PCSX2/pcsx2');
  });

  it('open, update and companion install toast a refusal and clear their busy state', async () => {
    const s = await loadedStore();
    api.launchEmulator.mockResolvedValueOnce('No ROM folder set');
    await s.openEmulator('RetroArch');
    expect(toasts).toContainEqual(['No ROM folder set', 'error']);
    expect(s.launching).toBeNull();
    api.updateEmulator.mockRejectedValueOnce(new Error('offline'));
    await s.updateEmulator('RetroArch');
    expect(s.updating).toBeNull();
    const companion = { profile: 'QtLauncher', entry: null, sourceId: 'shadps4/qt' } as never;
    await s.installCompanion(companion);
    expect(api.installEmulator).toHaveBeenCalledWith('shadps4/qt');
    expect(s.companionInstalling(companion)).toBe(false);
  });

  it('check for updates says how many were found', async () => {
    const s = await loadedStore();
    await s.checkForUpdates();
    expect(toasts.at(-1)?.[0]).toBe('Every emulator is up to date.');
    hasUpdate = () => true;
    await s.checkForUpdates();
    expect(toasts.at(-1)?.[0]).toBe('1 update available.');
  });

  it('removeEmulator answers whether it went', async () => {
    const s = await loadedStore();
    expect(await s.removeEmulator('RetroArch')).toBe(true);
    expect(toasts).toContainEqual(['RetroArch removed', undefined]);
    api.deleteEmulator.mockRejectedValueOnce(new Error('in use'));
    expect(await s.removeEmulator('RetroArch')).toBe(false);
    expect(s.removing).toBe(false);
  });

  it('setup defaults re-read the defaults until the watch stops', async () => {
    const s = await store();
    const stop = s.watchSetupDefaults();
    await Promise.resolve();
    handlers['setup-defaults']({ payload: null });
    expect(api.getLaunchDefaults).toHaveBeenCalledTimes(1);
    stop();
    await Promise.resolve();
    expect(handlers['setup-defaults']).toBeUndefined();
  });

  it('PS3 firmware install polls until RPCS3 reports it installed', async () => {
    const s = await store();
    vi.useFakeTimers();
    try {
      const run = s.installPs3Firmware('RPCS3');
      await vi.advanceTimersByTimeAsync(3000);
      await run;
    } finally {
      vi.useRealTimers();
    }
    expect(toasts[0][1]).toBe('success');
    expect(s.ps3Installed('RPCS3')).toBe(true);
    api.installPs3Firmware.mockResolvedValueOnce(false);
    await s.installPs3Firmware('RPCS3');
    expect(toasts.at(-1)?.[1]).toBe('error');
  });
});
