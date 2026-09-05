import { beforeEach, describe, expect, it, vi } from 'vitest';

const { ensureBackgroundVariant } = vi.hoisted(() => ({ ensureBackgroundVariant: vi.fn() }));
vi.mock('./api', () => ({ api: { ensureBackgroundVariant } }));

const { fade } = vi.hoisted(() => ({ fade: { value: 40 } }));
vi.mock('./stores/uiSettings.svelte', () => ({
  uiSettings: {
    get backgroundFade() {
      return fade.value;
    },
  },
}));

import { prefetchBackground, rememberVariant, VARIANT_MEMO_CAP, variantPaths } from './backgroundPrefetch';

const settled = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  ensureBackgroundVariant.mockReset();
  ensureBackgroundVariant.mockResolvedValue('/cache/a.bg.jpg');
  variantPaths.clear();
  fade.value = 40;
});

describe('prefetchBackground', () => {
  it('warms the first URL of the winning tier and memoises the path', async () => {
    prefetchBackground({
      fanart: [],
      screenshots: ['https://romm/shot-1.png', 'https://romm/shot-2.png'],
      cover: 'https://romm/cover.png',
    });
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/shot-1.png');
    await settled();
    expect(variantPaths.get('https://romm/shot-1.png')).toBe('/cache/a.bg.jpg');
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

  it('swallows a rejection instead of leaving it unhandled', async () => {
    ensureBackgroundVariant.mockRejectedValue(new Error('the image could not be decoded'));
    expect(() =>
      prefetchBackground({ fanart: ['https://romm/art.svg'], screenshots: [], cover: null })
    ).not.toThrow();
    expect(ensureBackgroundVariant).toHaveBeenCalledExactlyOnceWith('https://romm/art.svg');
    await settled();
    expect(variantPaths.has('https://romm/art.svg')).toBe(false);
  });
});

describe('rememberVariant', () => {
  it('drops the oldest entry once the cap is passed', () => {
    for (let i = 0; i < VARIANT_MEMO_CAP; i += 1) rememberVariant(`https://romm/${i}.png`, `/cache/${i}.bg.jpg`);
    expect(variantPaths.size).toBe(VARIANT_MEMO_CAP);

    rememberVariant('https://romm/new.png', '/cache/new.bg.jpg');
    expect(variantPaths.size).toBe(VARIANT_MEMO_CAP);
    expect(variantPaths.has('https://romm/0.png')).toBe(false);
    expect(variantPaths.has('https://romm/1.png')).toBe(true);
    expect(variantPaths.get('https://romm/new.png')).toBe('/cache/new.bg.jpg');
  });
});
