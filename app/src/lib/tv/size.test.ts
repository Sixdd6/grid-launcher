import { describe, expect, it } from 'vitest';
import { rootFontPx } from './size';

describe('rootFontPx (item 2; ruling 1)', () => {
  it('is 1/54 of the height: 20px at 1080p, 40 at 4K, 13.3 at 720p', () => {
    expect(rootFontPx(1920, 1080)).toBe(20);
    expect(rootFontPx(3840, 2160)).toBe(40);
    expect(rootFontPx(1280, 720)).toBeCloseTo(13.333);
  });
  it('is capped at 1/80 of the width on squarer screens', () => {
    expect(rootFontPx(1280, 1024)).toBe(16);
  });
});
