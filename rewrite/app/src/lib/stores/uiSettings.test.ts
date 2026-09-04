import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// `uiSettings.svelte.ts` is module-scoped state (see its own header comment),
// so each test needs a fresh module instance — `vi.resetModules()` plus a
// dynamic `import()` inside the test body, after the fakes below are wired
// with `vi.doMock`/`vi.stubGlobal`.

function fakeStorage(initial: Record<string, string> = {}): Storage {
  const store: Record<string, string> = { ...initial };
  return {
    getItem: (key: string) => (key in store ? store[key] : null),
    setItem: (key: string, value: string) => {
      store[key] = value;
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      for (const key of Object.keys(store)) delete store[key];
    },
    key: () => null,
    get length() {
      return Object.keys(store).length;
    },
  } as Storage;
}

function fakeMedia(matches: boolean): MediaQueryList {
  return {
    matches,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList;
}

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.doUnmock('../api');
});

describe('initUiSettings', () => {
  it('applies the localStorage mirror before the config promise resolves, then reconciles', async () => {
    const dataset: Record<string, string> = {};
    vi.stubGlobal('localStorage', fakeStorage({ 'grid.ui.theme': 'dark' }));
    vi.stubGlobal('document', { documentElement: { dataset } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });

    let resolveConfig!: (v: { theme: string; background_fade: number; card_size_library: string; card_size_server: string }) => void;
    const configPromise = new Promise<{ theme: string; background_fade: number; card_size_library: string; card_size_server: string }>((resolve) => {
      resolveConfig = resolve;
    });
    vi.doMock('../api', () => ({
      api: { getUiSettings: () => configPromise, setUiSettings: vi.fn() },
    }));

    const { initUiSettings, uiSettings } = await import('./uiSettings.svelte');
    const initPromise = initUiSettings();

    // Synchronous part of initUiSettings has already run: the mirror is
    // applied before the `await`, well before the config round-trip settles.
    expect(dataset.theme).toBe('dark');
    expect(uiSettings.theme).toBe('dark');

    resolveConfig({ theme: 'light', background_fade: 25, card_size_library: 'medium', card_size_server: 'medium' });
    await initPromise;

    // The config value disagrees with the mirror, so it wins once it loads.
    expect(dataset.theme).toBe('light');
    expect(uiSettings.theme).toBe('light');
  });

  it('does not reapply when the config agrees with the mirror', async () => {
    const dataset: Record<string, string> = {};
    vi.stubGlobal('localStorage', fakeStorage({ 'grid.ui.theme': 'dark' }));
    vi.stubGlobal('document', { documentElement: { dataset } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () => Promise.resolve({ theme: 'dark', background_fade: 25, card_size_library: 'medium', card_size_server: 'medium' }),
        setUiSettings: vi.fn(),
      },
    }));

    const { initUiSettings, uiSettings } = await import('./uiSettings.svelte');
    await initUiSettings();

    expect(dataset.theme).toBe('dark');
    expect(uiSettings.theme).toBe('dark');
  });

  it('falls back to the config value when there is no stored mirror', async () => {
    const dataset: Record<string, string> = {};
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () => Promise.resolve({ theme: 'light', background_fade: 25, card_size_library: 'medium', card_size_server: 'medium' }),
        setUiSettings: vi.fn(),
      },
    }));

    const { initUiSettings, uiSettings } = await import('./uiSettings.svelte');
    await initUiSettings();

    expect(dataset.theme).toBe('light');
    expect(uiSettings.theme).toBe('light');
  });
});

describe('setTheme', () => {
  it('mirrors the choice to localStorage once the save resolves', async () => {
    const dataset: Record<string, string> = {};
    const storage = fakeStorage({ 'grid.ui.theme': 'light' });
    vi.stubGlobal('localStorage', storage);
    vi.stubGlobal('document', { documentElement: { dataset } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () => Promise.resolve({ theme: 'light', background_fade: 25, card_size_library: 'medium', card_size_server: 'medium' }),
        setUiSettings: () => Promise.resolve(),
      },
    }));

    const { initUiSettings, setTheme } = await import('./uiSettings.svelte');
    await initUiSettings();
    await setTheme('dark');

    expect(dataset.theme).toBe('dark');
    expect(storage.getItem('grid.ui.theme')).toBe('dark');
  });

  it('leaves the mirror on the saved value when the save fails', async () => {
    const dataset: Record<string, string> = {};
    const storage = fakeStorage({ 'grid.ui.theme': 'light' });
    vi.stubGlobal('localStorage', storage);
    vi.stubGlobal('document', { documentElement: { dataset } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () => Promise.resolve({ theme: 'light', background_fade: 25, card_size_library: 'medium', card_size_server: 'medium' }),
        setUiSettings: () => Promise.reject(new Error('config is read-only')),
      },
    }));

    const { initUiSettings, setTheme } = await import('./uiSettings.svelte');
    await initUiSettings();
    await expect(setTheme('dark')).rejects.toThrow();

    // The attribute follows the click immediately, but the hint index.html
    // reads on the next launch still names what `config.toml` holds — no
    // pre-paint of a theme that was never saved.
    expect(dataset.theme).toBe('dark');
    expect(storage.getItem('grid.ui.theme')).toBe('light');
  });
});
