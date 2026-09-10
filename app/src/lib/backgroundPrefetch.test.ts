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
  dropPendingWarms,
  inFlightBuilds,
  PENDING_CAP,
  PREFETCH_CONCURRENCY,
  prefetchBackground,
  rememberVariant,
  resetPrefetchQueue,
  SCROLL_IDLE_MS,
  setScrollIdle,
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

  // Every queued target captured the sigma it was enqueued at. Building one
  // after the slider moved would make `remove_stale_variants` delete the
  // new-sigma file the display path had just built.
  it('drops the queued builds, so none runs at the sigma the user left', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY + 1; i += 1)
      warmBackground(cover(`https://romm/${i}.png`));
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(PREFETCH_CONCURRENCY);

    blur.value = 20; // the slider was released
    clearVariantMemo();

    // The three in flight finish and record under their own old key, which
    // is harmless — nothing reads it. The queued fourth never starts.
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1) builds.resolve(i);
    await settled();
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(PREFETCH_CONCURRENCY);
    expect(ensureBackgroundVariant.mock.calls.map((call) => call[0])).not.toContain(
      `https://romm/${PREFETCH_CONCURRENCY}.png`
    );

    // And the dropped warm is not remembered as refused: it builds at the
    // new sigma when the card is warmed again.
    warmBackground(cover(`https://romm/${PREFETCH_CONCURRENCY}.png`));
    expect(ensureBackgroundVariant).toHaveBeenLastCalledWith(
      `https://romm/${PREFETCH_CONCURRENCY}.png`,
      20
    );
  });
});

/** Resolves every build the queue has started, and every build those starts,
 *  until it settles. Returns the URLs that were actually built, in order. */
async function drainAll(builds: ReturnType<typeof deferredBuilds>): Promise<string[]> {
  for (let done = 0; done < ensureBackgroundVariant.mock.calls.length; done += 1) {
    builds.resolve(done);
    await settled();
  }
  return ensureBackgroundVariant.mock.calls.map((call) => call[0] as string);
}

describe('the queue depth cap', () => {
  it('sheds the oldest waiting warm once the queue is full', async () => {
    const builds = deferredBuilds();
    // Three fill the in-flight slots, the cap's worth wait, five overflow.
    const total = PREFETCH_CONCURRENCY + PENDING_CAP + 5;
    for (let i = 0; i < total; i += 1) warmBackground(cover(`https://romm/${i}.png`));

    const built = await drainAll(builds);
    expect(built).toHaveLength(total - 5);
    // The five oldest WAITING warms went; the three already in flight and
    // the newest entries — the cards nearest the viewport — stayed.
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      expect(built).toContain(`https://romm/${i}.png`);
    for (let i = PREFETCH_CONCURRENCY; i < PREFETCH_CONCURRENCY + 5; i += 1)
      expect(built).not.toContain(`https://romm/${i}.png`);
    expect(built).toContain(`https://romm/${total - 1}.png`);
  });

  it('lets a shed warm be warmed again — a drop is not a refusal', async () => {
    const builds = deferredBuilds();
    const total = PREFETCH_CONCURRENCY + PENDING_CAP + 5;
    for (let i = 0; i < total; i += 1) warmBackground(cover(`https://romm/${i}.png`));
    await drainAll(builds);

    const shed = `https://romm/${PREFETCH_CONCURRENCY}.png`;
    ensureBackgroundVariant.mockClear();
    warmBackground(cover(shed));
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith(shed, 12);
  });

  // A promoted warm IS a hover request: the pointer is on that card. It must
  // survive the cap the same way a hover request queued from cold does.
  it('never sheds a warm the hover path promoted', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      warmBackground(cover(`https://romm/busy-${i}.png`));
    warmBackground(cover('https://romm/promoted.png'));
    prefetchBackground(cover('https://romm/promoted.png')); // the pointer arrives
    for (let i = 0; i < PENDING_CAP + 5; i += 1) warmBackground(cover(`https://romm/${i}.png`));

    const built = await drainAll(builds);
    expect(built).toContain('https://romm/promoted.png');
  });

  it('never sheds a hover request, only warms', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      warmBackground(cover(`https://romm/busy-${i}.png`));
    prefetchBackground(cover('https://romm/hovered.png'));
    for (let i = 0; i < PENDING_CAP + 5; i += 1) warmBackground(cover(`https://romm/${i}.png`));

    const built = await drainAll(builds);
    expect(built).toContain('https://romm/hovered.png');
  });
});

describe('dropPendingWarms', () => {
  it('drops the queued warms a view left behind, keeping the hover request', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      warmBackground(cover(`https://romm/busy-${i}.png`));
    prefetchBackground(cover('https://romm/hovered.png'));
    warmBackground(cover('https://romm/warm-a.png'));
    warmBackground(cover('https://romm/warm-b.png'));

    dropPendingWarms();

    const built = await drainAll(builds);
    expect(built).toContain('https://romm/hovered.png');
    expect(built).not.toContain('https://romm/warm-a.png');
    expect(built).not.toContain('https://romm/warm-b.png');
    // In-flight builds are left to finish; their result is memoised.
    expect(built).toContain('https://romm/busy-0.png');
  });

  it('keeps a warm the hover path promoted', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      warmBackground(cover(`https://romm/busy-${i}.png`));
    warmBackground(cover('https://romm/promoted.png'));
    warmBackground(cover('https://romm/warm-a.png'));
    prefetchBackground(cover('https://romm/promoted.png')); // the pointer arrives

    dropPendingWarms();

    const built = await drainAll(builds);
    expect(built).toContain('https://romm/promoted.png');
    expect(built).not.toContain('https://romm/warm-a.png');
  });

  it('lets a dropped warm be warmed again when its card comes back', async () => {
    const builds = deferredBuilds();
    for (let i = 0; i < PREFETCH_CONCURRENCY; i += 1)
      warmBackground(cover(`https://romm/busy-${i}.png`));
    warmBackground(cover('https://romm/warm-a.png'));

    dropPendingWarms();
    await drainAll(builds);
    expect(inFlightBuilds()).toBe(0);
    ensureBackgroundVariant.mockClear();

    warmBackground(cover('https://romm/warm-a.png'));
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith(
      'https://romm/warm-a.png',
      12
    );
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

describe('the scroll-idle gate', () => {
  it('waits a quarter of a second of stillness before warming again', () => {
    expect(SCROLL_IDLE_MS).toBe(250);
  });

  it('starts no warm while the user is scrolling', () => {
    setScrollIdle(false);
    warmBackground(cover('https://romm/a.png'));

    expect(ensureBackgroundVariant).not.toHaveBeenCalled();
    expect(inFlightBuilds()).toBe(0);
  });

  it('starts the waiting warms once the scroll stops', () => {
    setScrollIdle(false);
    warmBackground(cover('https://romm/a.png'));
    warmBackground(cover('https://romm/b.png'));
    expect(ensureBackgroundVariant).not.toHaveBeenCalled();

    setScrollIdle(true);
    expect(ensureBackgroundVariant).toHaveBeenCalledTimes(2);
  });

  it('still starts a hover while the user is scrolling', () => {
    setScrollIdle(false);
    prefetchBackground(cover('https://romm/hovered.png'));

    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/hovered.png', 12);
  });

  it('does not let a hover drag the waiting warms out with it', () => {
    setScrollIdle(false);
    warmBackground(cover('https://romm/warm.png'));
    prefetchBackground(cover('https://romm/hovered.png'));

    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/hovered.png', 12);
  });

  it('keeps the warms it held back, rather than dropping them', async () => {
    setScrollIdle(false);
    warmBackground(cover('https://romm/held.png'));
    setScrollIdle(true);

    await settled();
    expect(variantPaths.get(variantKey(12, 'https://romm/held.png'))).toBe('/cache/a.bg.jpg');
  });

  it('a second pause changes nothing that is already in flight', () => {
    const builds = deferredBuilds();
    warmBackground(cover('https://romm/a.png'));
    expect(inFlightBuilds()).toBe(1);

    setScrollIdle(false);
    expect(inFlightBuilds()).toBe(1);
    builds.resolve(0);
  });
});
