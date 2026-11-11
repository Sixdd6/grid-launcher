import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlatformSetup, SetupOption } from '../api';

type Fn = ReturnType<typeof vi.fn>;

const opt = (source_id: string, name: string): SetupOption => ({
  source_id,
  name,
  kind: 'standalone',
  installed: false,
  server: null,
  needs_core: false,
});

function row(label: string, options: SetupOption[]): PlatformSetup {
  return { label, name: label, slug: label.toLowerCase(), rom_count: 4, options, recommended: options[0]?.source_id ?? null, core: null };
}

const ps2 = row('PlayStation 2', [opt('PCSX2/pcsx2', 'PCSX2 (Playstation 2)')]);
const dc = row('Dreamcast', [opt('redream/redream', 'Redream (Sega Dreamcast)')]);

describe('ChooseSystems', () => {
  let api: Record<string, Fn>;
  let toasts: string[];

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    api = {
      setupPlatformOptions: vi.fn(async () => [ps2, dc]),
      listCompatTools: vi.fn(async () => ({
        tools: [{ path: '/proton/GE', name: 'GE-Proton9', kind: 'proton', source: 'grid' }],
        default_tool: '/proton/GE',
      })),
      listCompatToolCatalog: vi.fn(async () => []),
      listPlatforms: vi.fn(async () => [
        { id: 1, name: 'Windows', slug: 'win', rom_count: 7, custom_name: null, display_name: 'Windows' },
      ]),
      startPlatformSetup: vi.fn(async () => ({ queued: ['x'], compat_set: false, errors: [] })),
      skipPlatformSetup: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../stores/toasts.svelte', () => ({ pushToast: (t: string) => toasts.push(t) }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('../stores/toasts.svelte');
  });

  async function chooser(windowsHost = false) {
    const { ChooseSystems } = await import('./chooseSystemsState.svelte');
    return new ChooseSystems(windowsHost);
  }

  it('loads the rows sorted, and keeps a load error', async () => {
    const cs = await chooser();
    await cs.load();
    expect(cs.rows?.map((r) => r.label)).toEqual(['Dreamcast', 'PlayStation 2']);
    api.setupPlatformOptions.mockRejectedValueOnce('offline');
    await cs.load();
    expect(cs.loadError).toBe('offline');
  });

  it('ticking pre-selects the recommendation and counts the install', async () => {
    const cs = await chooser();
    await cs.load();
    cs.tick(ps2);
    expect(cs.selection).toEqual({ 'PlayStation 2': 'PCSX2/pcsx2' });
    expect(cs.button).toEqual({ installs: 1, ticked: 1, proton: null });
    expect(cs.installNames()).toEqual({ 'PCSX2/pcsx2': 'PCSX2' });
    cs.tick(ps2);
    expect(cs.ticked).toBe(0);
  });

  it('the Windows row loads on Linux only and ticks the preselected Proton', async () => {
    const win = await chooser(true);
    await win.loadWindows();
    expect(api.listCompatTools).not.toHaveBeenCalled();
    const cs = await chooser();
    await cs.loadWindows();
    expect(cs.windowsLoaded).toBe(true);
    expect(cs.windowsGames).toBe(7);
    cs.tickWindows();
    expect(cs.proton).toBe(cs.protonDefault);
    expect(cs.proton).not.toBeNull();
    cs.tickWindows();
    expect(cs.proton).toBeNull();
  });

  it('install answers what started, or null with the refusal', async () => {
    const cs = await chooser();
    await cs.load();
    cs.tick(dc);
    const done = await cs.install();
    expect(done?.names).toEqual({ 'redream/redream': 'Redream' });
    expect(api.startPlatformSetup).toHaveBeenCalledWith([{ platform: 'Dreamcast', source_id: 'redream/redream' }], null);
    api.startPlatformSetup.mockRejectedValueOnce('no server');
    expect(await cs.install()).toBeNull();
    expect(cs.startError).toBe('no server');
    expect(cs.busy).toBe(false);
  });

  it('an installed Proton pick with nothing queued says which one Windows games use', async () => {
    api.startPlatformSetup.mockResolvedValueOnce({ queued: [], compat_set: true, errors: [] });
    const cs = await chooser();
    await cs.load();
    await cs.loadWindows();
    cs.tickWindows();
    await cs.install();
    expect(toasts).toHaveLength(1);
    expect(toasts[0]).toMatch(/^Windows games will use /);
  });

  it('skip records the skip, and a failure to record it is not an error', async () => {
    api.skipPlatformSetup.mockRejectedValueOnce(new Error('read-only config'));
    const cs = await chooser();
    await cs.skip();
    expect(cs.busy).toBe(false);
    expect(api.skipPlatformSetup).toHaveBeenCalledTimes(1);
  });
});
