import { describe, expect, it, vi } from 'vitest';

vi.mock('../fileSrc', () => ({ fileSrc: (p: string) => `asset://${p}` }));

import { storeArtSrc } from './storeArt';

describe('storeArtSrc', () => {
  it('fetches a web address through ensure_image and never hands it to the page', async () => {
    const ensure = vi.fn(async () => '/data/stores/art/ab.jpg');
    expect(await storeArtSrc('https://images.gog-statics.com/a.jpg', ensure)).toBe('asset:///data/stores/art/ab.jpg');
    expect(ensure).toHaveBeenCalledWith('https://images.gog-statics.com/a.jpg');
  });

  it('answers null for a failed fetch or no address', async () => {
    expect(await storeArtSrc('https://x/a.jpg', () => Promise.reject(new Error('no')))).toBeNull();
    expect(await storeArtSrc('  ', vi.fn())).toBeNull();
  });

  it('keeps a data URL and serves a cached path through the asset protocol', async () => {
    const ensure = vi.fn();
    expect(await storeArtSrc('data:image/png;base64,AA', ensure)).toBe('data:image/png;base64,AA');
    expect(await storeArtSrc('/cache/cover.jpg', ensure)).toBe('asset:///cache/cover.jpg');
    expect(ensure).not.toHaveBeenCalled();
  });
});
