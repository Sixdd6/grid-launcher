import { beforeEach, describe, expect, it, vi } from 'vitest';

const { ensureBackgroundVariant } = vi.hoisted(() => ({ ensureBackgroundVariant: vi.fn() }));
vi.mock('./api', () => ({ api: { ensureBackgroundVariant } }));

const { fade, blur } = vi.hoisted(() => ({ fade: { value: 40 }, blur: { value: 12 } }));
vi.mock('./stores/uiSettings.svelte', () => ({
  uiSettings: {
    get backgroundFade() {
      return fade.value;
    },
    get backgroundBlur() {
      return blur.value;
    },
  },
}));

import {
  clearVariantMemo,
  prefetchBackground,
  rememberVariant,
  resetWarmQueue,
  VARIANT_MEMO_CAP,
  variantKey,
  variantPaths,
  WARM_CONCURRENCY,
  warmBackground,
} from './backgroundPrefetch';

const settled = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  ensureBackgroundVariant.mockReset();
  ensureBackgroundVariant.mockResolvedValue('/cache/a.bg.jpg');
  variantPaths.clear();
  resetWarmQueue();
  fade.value = 40;
  blur.value = 12;
});

/** A build the test finishes by hand, so "two in flight" is observable. */
function deferredBuilds(): { resolve: (index: number, path?: string) => void; reject: (index: number) => void } {
  const settlers: { resolve: (path: string) => void; reject: (err: Error) => void }[] = [];
  ensureBackgroundVariant.mockImplementation(
    () => new Promise<string>((resolve, reject) => settlers.push({ resolve, reject }))
  );
  return {
    resolve: (index, path = `/cache/${index}.bg.jpg`) => settlers[index].resolve(path),
    reject: (index) => settlers[index].reject(new Error('offline')),
  };
}

const cover = (url: string) => ({ fanart: [], screenshots: [], cover: url });

describe('prefetchBackground', () => {
  it('warms the first URL of the winning tier and memoises the path', async () => {
    prefetchBackground({
      fanart: [],
      screenshots: ['https://romm/shot-1.png', 'https://romm/shot-2.png'],
      cover: 'https://romm/cover.png',
    });
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/shot-1.png', 12);
    await settled();
    expect(variantPaths.get(variantKey(12, 'https://romm/shot-1.png'))).toBe('/cache/a.bg.jpg');
  });

  it('asks for nothing when the subject has no art', () => {
    prefetchBackground({ fanart: [], screenshots: [], cover: null });
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('asks for nothing while the background art is switched off', () => {
    fade.value = 0;
    prefetchBackground({ fanart: [], screenshots: [], cover: 'https://romm/cover.png' });
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('does not ask twice for a URL it has already resolved', async () => {
    prefetchBackground({ fanart: [], screenshots: [], cover: 'https://romm/cover.png' });
    await settled();
    prefetchBackground({ fanart: [], screenshots: [], cover: 'https://romm/cover.png' });
    expect(ensureBackgroundVariant).toHaveBeenCalledOnce();
  });

  it('asks again after the blur level changes — the sigma names a different file', async () => {
    const subject = { fanart: [], screenshots: [], cover: 'https://romm/cover.png' };
    prefetchBackground(subject);
    await settled();
    prefetchBackground(subject);
    expect(ensureBackgroundVariant).toHaveBeenCalledOnce();

    blur.value = 0;
    prefetchBackground(subject);
    expect(ensureBackgroundVariant).toHaveBeenLastCalledWith('https://romm/cover.png', 0);
    await settled();
    expect(variantPaths.get(variantKey(0, 'https://romm/cover.png'))).toBe('/cache/a.bg.jpg');
    expect(variantPaths.has(variantKey(12, 'https://romm/cover.png'))).toBe(true);
  });

  it('swallows a rejection instead of leaving it unhandled', async () => {
    ensureBackgroundVariant.mockRejectedValue(new Error('the image could not be decoded'));
    expect(() =>
      prefetchBackground({ fanart: ['https://romm/art.svg'], screenshots: [], cover: null })
    ).not.toThrow();
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/art.svg', 12);
    await settled();
    expect(variantPaths.has(variantKey(12, 'https://romm/art.svg'))).toBe(false);
  });
});

describe('rememberVariant', () => {
  it('drops the oldest entry once the cap is passed', () => {
    for (let i = 0; i < VARIANT_MEMO_CAP; i += 1)
      rememberVariant(variantKey(12, `https://romm/${i}.png`), `/cache/${i}.bg.jpg`);
    expect(variantPaths.size).toBe(VARIANT_MEMO_CAP);

    rememberVariant(variantKey(12, 'https://romm/new.png'), '/cache/new.bg.jpg');
    expect(variantPaths.size).toBe(VARIANT_MEMO_CAP);
    expect(variantPaths.has(variantKey(12, 'https://romm/0.png'))).toBe(false);
    expect(variantPaths.has(variantKey(12, 'https://romm/1.png'))).toBe(true);
    expect(variantPaths.get(variantKey(12, 'https://romm/new.png'))).toBe('/cache/new.bg.jpg');
  });
});

describe('variantKey', () => {
  it('separates the sigma from the URL so two levels of one image differ', () => {
    expect(variantKey(12, 'https://romm/a.png')).not.toBe(variantKey(0, 'https://romm/a.png'));
    expect(variantKey(12, 'https://romm/a.png')).toBe(variantKey(12, 'https://romm/a.png'));
  });
});

describe('clearVariantMemo', () => {
  it('drops paths memoised at another sigma, so a returned-to blur re-fetches', () => {
    rememberVariant(variantKey(12, 'https://romm/cover.png'), '/cache/a.bg12.jpg');
    rememberVariant(variantKey(20, 'https://romm/cover.png'), '/cache/a.bg20.jpg');

    clearVariantMemo();

    expect(variantPaths.size).toBe(0);
  });
});

describe('warmBackground', () => {
  it('builds the first URL through the same memo key the hover prefetch uses', async () => {
    warmBackground({
      fanart: [],
      screenshots: ['https://romm/shot-1.png'],
      cover: 'https://romm/cover.png',
    });
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/shot-1.png', 12);
    await settled();
    expect(variantPaths.get(variantKey(12, 'https://romm/shot-1.png'))).toBe('/cache/a.bg.jpg');
  });

  it('asks once for a URL two cards share', () => {
    deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledOnce();
  });

  it('skips a URL the memo already holds', () => {
    rememberVariant(variantKey(12, 'https://romm/a.png'), '/cache/a.bg.jpg');
    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('asks for nothing while the background art is switched off', () => {
    fade.value = 0;
    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('asks for nothing when the subject has no art', () => {
    warmBackground({ fanart: [], screenshots: [], cover: null });
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('keeps at most WARM_CONCURRENCY builds in flight and starts the next as one resolves', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    warmBackground(cover('https://romm/b.png'));
    warmBackground(cover('https://romm/c.png'));

    expect(WARM_CONCURRENCY).toBe(2);
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(2);
    expect(ensureBackgroundVariant.mock.calls.map((c) => c[0])).toEqual([
      'https://romm/a.png',
      'https://romm/b.png',
    ]);

    builds.resolve(0);
    await settled();
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(3);
    expect(ensureBackgroundVariant).toHaveBeenLastCalledWith('https://romm/c.png', 12);
    expect(variantPaths.get(variantKey(12, 'https://romm/a.png'))).toBe('/cache/0.bg.jpg');
  });

  it('drops a refused warm instead of asking for it again', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    warmBackground(cover('https://romm/b.png'));
    warmBackground(cover('https://romm/c.png'));

    builds.reject(0);
    await settled();
    // The failure freed a slot for `c`, and nothing re-queued `a`.
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(3);
    expect(variantPaths.has(variantKey(12, 'https://romm/a.png'))).toBe(false);

    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(3);
  });

  it('forgets what it warmed when the memo is cleared, so a new sigma warms again', async () => {
    warmBackground(cover('https://romm/a.png'));
    await settled();
    clearVariantMemo();
    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(2);
  });
});
