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

describe('toggleTheme', () => {
  function fakeClassList() {
    const set = new Set<string>();
    return {
      set,
      add: (c: string) => void set.add(c),
      remove: (c: string) => void set.delete(c),
    };
  }

  async function loadStore(opts: { theme: string; prefersDark: boolean; reducedMotion: boolean }) {
    const dataset: Record<string, string> = {};
    const classList = fakeClassList();
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset, classList } });
    vi.stubGlobal('window', {
      matchMedia: (q: string) => fakeMedia(q.includes('reduced-motion') ? opts.reducedMotion : opts.prefersDark),
    });
    const setUiSettings = vi.fn().mockResolvedValue(undefined);
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () =>
          Promise.resolve({ theme: opts.theme, background_fade: 50, card_size_library: 'medium', card_size_server: 'medium' }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { ...store, dataset, classList, setUiSettings };
  }

  afterEach(() => {
    vi.useRealTimers();
  });

  it('switches a followed dark system theme to an explicit light one and saves it', async () => {
    const { toggleTheme, uiSettings, dataset, setUiSettings } = await loadStore({
      theme: 'system',
      prefersDark: true,
      reducedMotion: false,
    });
    await toggleTheme();
    expect(uiSettings.theme).toBe('light');
    expect(dataset.theme).toBe('light');
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme: 'light' }));
  });

  it('switches light to dark', async () => {
    const { toggleTheme, uiSettings } = await loadStore({ theme: 'light', prefersDark: true, reducedMotion: false });
    await toggleTheme();
    expect(uiSettings.theme).toBe('dark');
  });

  it('eases the colours only for the switch window', async () => {
    vi.useFakeTimers();
    const { toggleTheme, classList } = await loadStore({ theme: 'light', prefersDark: false, reducedMotion: false });
    await toggleTheme();
    expect(classList.set.has('theming')).toBe(true);
    vi.advanceTimersByTime(320);
    expect(classList.set.has('theming')).toBe(false);
  });

  it('switches instantly under reduced motion', async () => {
    const { toggleTheme, classList, uiSettings } = await loadStore({
      theme: 'light',
      prefersDark: false,
      reducedMotion: true,
    });
    await toggleTheme();
    expect(uiSettings.theme).toBe('dark');
    expect(classList.set.has('theming')).toBe(false);
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

  it('off writes a full fade, and on restores the value the slider last held', async () => {
    const { store, setUiSettings } = await loadStore(40);
    await store.setBackgroundEnabled(false);
    expect(store.uiSettings.backgroundFade).toBe(100);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 100 }));

    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(40);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 40 }));
  });

  it('on with a config that was already off uses the design default', async () => {
    const { store } = await loadStore(100);
    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(50);
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
    expect(store.uiSettings.backgroundBlur).toBe(2);
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
      card_labels: 'always',
      card_display: 'rows',
      tv_card_size: 'medium',
      tv_card_labels: 'always',
      hardware_acceleration: true,
      open_on: 'automatic',
      tv_prompts: 'match',
      tv_assignments: {},
      tv_welcomed: false,
      video_volume: 50,
    });
  });

  // Finding 1 of the round-5 review: the backend keeps ONE variant per
  // source, so a path memoised at the old sigma names a deleted file. The
  // commit must drop the memo before the store change re-runs the fetch.
  it('clears the variant memo so a returned-to sigma is re-fetched', async () => {
    const { store } = await loadStore(12);
    const prefetch = await import('../backgroundPrefetch');
    prefetch.rememberVariant(prefetch.variantKey(12, 'https://romm/cover.png'), '/cache/a.bg12.jpg');

    await store.commitBackgroundBlur(20);

    expect(prefetch.variantPaths.size).toBe(0);
  });

  it('keeps the sigma when another writer saves', async () => {
    const { store, setUiSettings } = await loadStore(12);
    await store.commitBackgroundBlur(18);
    await store.setCardSize('library', 'large');
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_blur: 18 }));
  });
});

describe('game labels', () => {
  async function loadStore(labels: unknown) {
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
            background_blur: 2,
            card_size_library: 'medium',
            card_size_server: 'medium',
            card_labels: labels,
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('loads the stored mode, and a config without one shows labels always', async () => {
    expect((await loadStore('hover')).store.uiSettings.cardLabels).toBe('hover');
    expect((await loadStore(undefined)).store.uiSettings.cardLabels).toBe('always');
  });

  it('applies a change at once and saves it with the rest of the settings', async () => {
    const { store, setUiSettings } = await loadStore('always');
    await store.setCardLabels('hover');
    expect(store.uiSettings.cardLabels).toBe('hover');
    expect(setUiSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({ card_labels: 'hover', background_fade: 25 }),
    );
  });
});

describe("TV mode's card size and game labels", () => {
  async function loadStore(stored: Record<string, unknown>) {
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
            background_blur: 2,
            card_size_library: 'small',
            card_size_server: 'large',
            card_labels: 'hover',
            ...stored,
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('loads the stored values, and a config without them is Medium and Always visible', async () => {
    const { store } = await loadStore({ tv_card_size: 'large', tv_card_labels: 'focus' });
    expect(store.uiSettings.tvCardSize).toBe('large');
    expect(store.uiSettings.tvCardLabels).toBe('focus');
    const fresh = (await loadStore({})).store;
    expect(fresh.uiSettings.tvCardSize).toBe('medium');
    expect(fresh.uiSettings.tvCardLabels).toBe('always');
  });

  it("never takes the desktop's values", async () => {
    const { store } = await loadStore({ tv_card_labels: 'hover' });
    expect(store.uiSettings.tvCardSize).toBe('medium');
    expect(store.uiSettings.tvCardLabels).toBe('always');
  });

  it("saves a change without touching the desktop's keys", async () => {
    const { store, setUiSettings } = await loadStore({});
    await store.setTvCardSize('small');
    await store.setTvCardLabels('focus');
    expect(store.uiSettings.tvCardSize).toBe('small');
    expect(store.uiSettings.tvCardLabels).toBe('focus');
    expect(store.uiSettings.cardSizeLibrary).toBe('small');
    expect(store.uiSettings.cardSizeServer).toBe('large');
    expect(store.uiSettings.cardLabels).toBe('hover');
    expect(setUiSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({
        tv_card_size: 'small',
        tv_card_labels: 'focus',
        card_size_library: 'small',
        card_size_server: 'large',
        card_labels: 'hover',
      }),
    );
  });
});

describe('hardware acceleration', () => {
  async function loadStore(stored: unknown) {
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
            background_blur: 2,
            card_size_library: 'medium',
            card_size_server: 'medium',
            hardware_acceleration: stored,
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('is on unless the config turned it off', async () => {
    expect((await loadStore(undefined)).store.uiSettings.hardwareAcceleration).toBe(true);
    expect((await loadStore(false)).store.uiSettings.hardwareAcceleration).toBe(false);
  });

  it('saves a change with the rest of the settings', async () => {
    const { store, setUiSettings } = await loadStore(true);
    await store.setHardwareAcceleration(false);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ hardware_acceleration: false }));
  });
});

describe('Open GRID on and the TV keys', () => {
  async function loadStore(stored: Record<string, unknown>, rejects = false) {
    vi.resetModules();
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset: {} } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    const setUiSettings = vi.fn().mockResolvedValue(undefined);
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () =>
          rejects
            ? Promise.reject(new Error('no config'))
            : Promise.resolve({
                theme: 'system',
                background_fade: 25,
                background_blur: 2,
                card_size_library: 'medium',
                card_size_server: 'medium',
                ...stored,
              }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    expect(store.uiSettings.loaded).toBe(false);
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('loads the stored values, normalised, and marks the store loaded', async () => {
    const { store } = await loadStore({
      open_on: 'library',
      tv_prompts: 'nintendo',
      tv_assignments: { aa: 'nintendo', bb: 'sega' },
    });
    expect(store.uiSettings.loaded).toBe(true);
    expect(store.uiSettings.openOn).toBe('library');
    expect(store.uiSettings.tvPrompts).toBe('nintendo');
    expect(store.uiSettings.tvAssignments).toEqual({ aa: 'nintendo' });
  });

  it('falls back to the defaults for a config written before these keys', async () => {
    const { store } = await loadStore({});
    expect(store.uiSettings.openOn).toBe('automatic');
    expect(store.uiSettings.tvPrompts).toBe('match');
    expect(store.uiSettings.tvAssignments).toEqual({});
    expect(store.uiSettings.tvWelcomed).toBe(false);
  });

  it('remembers the welcome card was shown, once (user decision 2026-11-10)', async () => {
    const { store, setUiSettings } = await loadStore({ tv_assignments: { aa: 'xbox' } });
    await store.setTvWelcomed();
    expect(store.uiSettings.tvWelcomed).toBe(true);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ tv_welcomed: true, tv_assignments: { aa: 'xbox' } }));
    await store.setTvWelcomed();
    expect(setUiSettings).toHaveBeenCalledTimes(1);
  });

  it('reads the video volume back, clamped, with 50 for an old config (user decision 2026-11-10)', async () => {
    expect((await loadStore({})).store.uiSettings.videoVolume).toBe(50);
    expect((await loadStore({ video_volume: 80 })).store.uiSettings.videoVolume).toBe(80);
    expect((await loadStore({ video_volume: 400 })).store.uiSettings.videoVolume).toBe(100);
  });

  it('applies a volume change at once and writes it once, 500 ms after the last change', async () => {
    const { store, setUiSettings } = await loadStore({});
    vi.useFakeTimers();
    try {
      store.setVideoVolume(60);
      store.setVideoVolume(70);
      expect(store.uiSettings.videoVolume).toBe(70);
      vi.advanceTimersByTime(499);
      expect(setUiSettings).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(setUiSettings).toHaveBeenCalledTimes(1);
      expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ video_volume: 70 }));
      store.setVideoVolume(70);
      vi.advanceTimersByTime(1000);
      expect(setUiSettings).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('reads tv_welcomed back', async () => {
    const { store } = await loadStore({ tv_welcomed: true });
    expect(store.uiSettings.tvWelcomed).toBe(true);
  });

  it('is loaded even when the read fails', async () => {
    const { store } = await loadStore({}, true);
    expect(store.uiSettings.loaded).toBe(true);
    expect(store.uiSettings.openOn).toBe('automatic');
  });

  it('saves Open GRID on with the rest of the settings, keeping the TV keys', async () => {
    const { store, setUiSettings } = await loadStore({ tv_assignments: { aa: 'xbox' } });
    await store.setOpenOn('discover');
    expect(store.uiSettings.openOn).toBe('discover');
    expect(setUiSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({ open_on: 'discover', tv_prompts: 'match', tv_assignments: { aa: 'xbox' } }),
    );
  });
});
