import { describe, expect, it } from 'vitest';
import { coverFit } from './letterbox';

describe('coverFit', () => {
  it('fits a 3:4 cover in a 3:4 frame, at any size', () => {
    expect(coverFit(264, 352, 150, 200)).toBe('fits');
    expect(coverFit(600, 800, 210, 280)).toBe('fits');
  });

  it('ignores a sub-pixel difference', () => {
    // 263×352 in 150×200 leaves under a pixel.
    expect(coverFit(263, 352, 150, 200)).toBe('fits');
  });

  it('crops near-3:4 box art instead of letterboxing it', () => {
    // ScreenScraper's 198×272 leaves ~4.6px of bars in a 160px-wide card.
    expect(coverFit(198, 272, 160, 213.33)).toBe('crop');
    // A little narrower (0.711) and a little wider (0.776) than 3:4.
    expect(coverFit(170, 239, 160, 213.33)).toBe('crop');
    expect(coverFit(170, 219, 160, 213.33)).toBe('crop');
  });

  it('letterboxes square, wide and much taller art', () => {
    expect(coverFit(512, 512, 150, 200)).toBe('letterbox');
    expect(coverFit(640, 360, 150, 200)).toBe('letterbox');
    expect(coverFit(128, 96, 160, 213.33)).toBe('letterbox');
    expect(coverFit(300, 600, 150, 200)).toBe('letterbox');
  });

  it('letterboxes when a size is unknown', () => {
    expect(coverFit(0, 0, 150, 200)).toBe('letterbox');
    expect(coverFit(264, 352, 0, 0)).toBe('letterbox');
  });
});
