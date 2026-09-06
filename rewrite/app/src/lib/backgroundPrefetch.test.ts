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
  inFlightBuilds,
  PREFETCH_CONCURRENCY,
  prefetchBackground,
  rememberVariant,
  resetPrefetchQueue,
  VARIANT_MEMO_CAP,
  variantKey,
  variantPaths,
  warmBackground,
} from './backgroundPrefetch';

const settled = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  ensureBackgroundVariant.mockReset();
  ensureBackgroundVariant.mockResolvedValue('/cache/a.bg.jpg');
  variantPaths.clear();
  resetPrefetchQueue();
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

  it('reports the art as switched off, so the caller keeps watching the card', () => {
    fade.value = 0;
    expect(warmBackground(cover('https://romm/a.png'))).toBe(false);
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('reports a card with no art as dealt with — nothing will ever build', () => {
    expect(warmBackground({ fanart: [], screenshots: [], cover: null })).toBe(true);
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
  });

  it('drops a refused warm instead of asking for it again', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));

    builds.reject(0);
    await settled();
    expect(variantPaths.has(variantKey(12, 'https://romm/a.png'))).toBe(false);

    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledOnce();
  });

  it('forgets what it asked for when the memo is cleared, so a new sigma builds again', async () => {
    warmBackground(cover('https://romm/a.png'));
    await settled();
    clearVariantMemo();
    warmBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(2);
  });
});

/** One queue serves both callers, so the cap is a real ceiling rather than a
 *  per-caller suggestion. */
describe('the shared build queue', () => {
  const asked = () => ensureBackgroundVariant.mock.calls.map((c) => c[0] as string);

  it('keeps at most PREFETCH_CONCURRENCY builds in flight across both callers', () => {
    deferredBuilds();
    expect(PREFETCH_CONCURRENCY).toBe(3);

    warmBackground(cover('https://romm/w1.png'));
    warmBackground(cover('https://romm/w2.png'));
    prefetchBackground(cover('https://romm/h1.png'));
    warmBackground(cover('https://romm/w3.png'));
    prefetchBackground(cover('https://romm/h2.png'));

    expect(inFlightBuilds()).toBe(3);
    expect(asked()).toEqual(['https://romm/w1.png', 'https://romm/w2.png', 'https://romm/h1.png']);
  });

  it('lets a hover jump ahead of the warms still waiting', async () => {
    const builds = deferredBuilds();
    for (const n of [1, 2, 3, 4, 5]) warmBackground(cover(`https://romm/w${n}.png`));
    prefetchBackground(cover('https://romm/hovered.png'));
    expect(asked()).toHaveLength(3);

    builds.resolve(0);
    await settled();
    // The hovered card, not w4 — it was pushed to the front of the pending
    // list, ahead of every warm that had not started.
    expect(ensureBackgroundVariant).toHaveBeenLastCalledWith('https://romm/hovered.png', 12);
  });

  it('promotes a warm that has not started when the same card is hovered', async () => {
    const builds = deferredBuilds();
    // Three slots taken, so w4 and w5 are only queued.
    for (const n of [1, 2, 3, 4, 5]) warmBackground(cover(`https://romm/w${n}.png`));
    expect(asked()).toHaveLength(3);

    // The user scrolled past w5 and is now looking at it, before its turn.
    prefetchBackground(cover('https://romm/w5.png'));
    expect(asked()).toHaveLength(3); // still nothing new asked for

    builds.resolve(0);
    await settled();
    expect(ensureBackgroundVariant).toHaveBeenLastCalledWith('https://romm/w5.png', 12);
  });

  it('leaves a build already in flight where it is when it is hovered', () => {
    deferredBuilds();
    for (const n of [1, 2, 3, 4]) warmBackground(cover(`https://romm/w${n}.png`));

    prefetchBackground(cover('https://romm/w1.png'));
    // No second ask for a build already running, and the queue is untouched:
    // w4 is still the next in line.
    expect(asked()).toEqual([
      'https://romm/w1.png',
      'https://romm/w2.png',
      'https://romm/w3.png',
    ]);
    expect(inFlightBuilds()).toBe(3);
  });

  it('does not retry a build that already failed, even on a hover', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    builds.reject(0);
    await settled();

    prefetchBackground(cover('https://romm/a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledOnce();
  });

  it('starts the next build as one resolves, and memoises the finished path', async () => {
    const builds = deferredBuilds();
    for (const n of [1, 2, 3, 4]) warmBackground(cover(`https://romm/w${n}.png`));

    builds.resolve(0);
    await settled();
    expect(asked()).toHaveLength(4);
    expect(inFlightBuilds()).toBe(3);
    expect(variantPaths.get(variantKey(12, 'https://romm/w1.png'))).toBe('/cache/0.bg.jpg');
  });

  it('returns to zero in flight once every build has settled, refusals included', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    prefetchBackground(cover('https://romm/b.png'));
    expect(inFlightBuilds()).toBe(2);

    builds.resolve(0);
    builds.reject(1);
    await settled();
    expect(inFlightBuilds()).toBe(0);
  });

  it('takes no slot for an entry another build memoised while it waited', async () => {
    const builds = deferredBuilds();
    for (const n of [1, 2, 3]) warmBackground(cover(`https://romm/w${n}.png`));
    warmBackground(cover('https://romm/queued.png'));

    // The waiting entry's path arrives from somewhere else (the swap path
    // memoises every build it makes) before its turn comes up.
    rememberVariant(variantKey(12, 'https://romm/queued.png'), '/cache/queued.bg.jpg');
    builds.resolve(0);
    await settled();

    expect(asked()).toEqual([
      'https://romm/w1.png',
      'https://romm/w2.png',
      'https://romm/w3.png',
    ]);
    // w1 finished, w2 and w3 are still out; the memoised entry took no slot.
    expect(inFlightBuilds()).toBe(2);
  });

  it('does not let builds outstanding across a reset drive the count negative', async () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    warmBackground(cover('https://romm/b.png'));

    resetPrefetchQueue();
    builds.resolve(0);
    builds.resolve(1);
    await settled();

    expect(inFlightBuilds()).toBe(0);
  });
});
