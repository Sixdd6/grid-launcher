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

describe('setBackgroundEnabled', () => {
  async function loadStore(fade: number) {
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset: {} } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    const setUiSettings = vi.fn().mockResolvedValue(undefined);
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () =>
          Promise.resolve({
            theme: 'system',
            background_fade: fade,
            background_blur: 12,
            card_size_library: 'medium',
            card_size_server: 'medium',
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('off writes fade 0, and on restores the value the slider last held', async () => {
    const { store, setUiSettings } = await loadStore(40);
    await store.setBackgroundEnabled(false);
    expect(store.uiSettings.backgroundFade).toBe(0);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 0 }));

    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(40);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 40 }));
  });

  it('on with a config that was already 0 uses the design default', async () => {
    const { store } = await loadStore(0);
    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(25);
  });

  it('a slider drag updates what "on" restores', async () => {
    const { store } = await loadStore(25);
    store.previewBackgroundFade(55);
    await store.setBackgroundEnabled(false);
    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(55);
  });
});

describe('background blur', () => {
  async function loadStore(blur: unknown) {
    // Fresh module per call: the store is module-scoped state, so two loads
    // inside one test would otherwise share the first load's values.
    vi.resetModules();
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset: {} } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    const setUiSettings = vi.fn().mockResolvedValue(undefined);
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () =>
          Promise.resolve({
            theme: 'system',
            background_fade: 25,
            background_blur: blur,
            card_size_library: 'medium',
            card_size_server: 'medium',
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('loads the stored sigma', async () => {
    const { store } = await loadStore(30);
    expect(store.uiSettings.backgroundBlur).toBe(30);
  });

  it('clamps a stored value a newer build wrote out of range', async () => {
    expect((await loadStore(99)).store.uiSettings.backgroundBlur).toBe(40);
    expect((await loadStore(-3)).store.uiSettings.backgroundBlur).toBe(0);
  });

  it('falls back to the default when the config has no sigma at all', async () => {
    const { store } = await loadStore(undefined);
    expect(store.uiSettings.backgroundBlur).toBe(12);
  });

  // Commit-on-release: every distinct sigma is a full backend rebuild, so
  // there is no drag preview to persist separately.
  it('applies the new sigma and persists the whole payload', async () => {
    const { store, setUiSettings } = await loadStore(12);
    await store.commitBackgroundBlur(0);
    expect(store.uiSettings.backgroundBlur).toBe(0);
    expect(setUiSettings).toHaveBeenLastCalledWith({
      theme: 'system',
      background_fade: 25,
      background_blur: 0,
      card_size_library: 'medium',
      card_size_server: 'medium',
    });
  });

  it('keeps the sigma when another writer saves', async () => {
    const { store, setUiSettings } = await loadStore(12);
    await store.commitBackgroundBlur(18);
    await store.setCardSize('library', 'large');
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_blur: 18 }));
  });
});
