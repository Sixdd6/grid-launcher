import { describe, expect, it } from 'vitest';

import { coverFallback, coverPair, pickCover, SMALL_COVER_MAX_PX } from './cover';
import { cardMinPx, targetRowPx } from './size';
import { ROW_MAX_SCALE } from './layout';

const pair = coverPair('/small.png', '/large.png');

describe('coverPair', () => {
  it('treats an empty URL as no cover', () => {
    expect(coverPair('', '')).toEqual({ small: null, large: null });
    expect(coverPair(null, undefined)).toEqual({ small: null, large: null });
    expect(coverPair('/s', '/l')).toEqual({ small: '/s', large: '/l' });
  });
});

describe('pickCover', () => {
  it('shows the small cover while the card is drawn no wider than it', () => {
    expect(pickCover(pair, SMALL_COVER_MAX_PX, 1)).toBe('/small.png');
    expect(pickCover(pair, 100, 2)).toBe('/small.png');
  });

  it('shows the large cover once the card is drawn wider than the small one', () => {
    expect(pickCover(pair, SMALL_COVER_MAX_PX + 1, 1)).toBe('/large.png');
    expect(pickCover(pair, 101, 2)).toBe('/large.png');
  });

  it('keeps narrow portrait cards cheap on a 1x display, and only those', () => {
    // The widest a portrait card gets in Equal height: the row at its
    // tallest, times the 3:4 shape.
    const widest = (size: 'small' | 'medium' | 'large') => targetRowPx(size) * ROW_MAX_SCALE * 0.75;
    expect(pickCover(pair, widest('small'), 1)).toBe('/small.png');
    expect(pickCover(pair, cardMinPx('small'), 1)).toBe('/small.png');
    expect(pickCover(pair, cardMinPx('small'), 2)).toBe('/large.png');
    expect(pickCover(pair, cardMinPx('medium'), 1.5)).toBe('/large.png');
    expect(pickCover(pair, widest('medium'), 1)).toBe('/large.png');
    expect(pickCover(pair, widest('large'), 1)).toBe('/large.png');
    // A wide cover in a Small row is wider than the small cover.
    expect(pickCover(pair, targetRowPx('small') * 2, 1)).toBe('/large.png');
  });

  it('keeps the large cover once it is on disk, even for a narrow card', () => {
    expect(pickCover(pair, 100, 1, true)).toBe('/large.png');
  });

  it('shows whichever cover exists when only one does', () => {
    expect(pickCover(coverPair('/small.png', null), 500, 2)).toBe('/small.png');
    expect(pickCover(coverPair(null, '/large.png'), 50, 1)).toBe('/large.png');
    expect(pickCover(coverPair(null, null), 500, 2)).toBeNull();
  });

  it('reads a broken pixel ratio as 1', () => {
    expect(pickCover(pair, 150, Number.NaN)).toBe('/small.png');
    expect(pickCover(pair, 150, 0)).toBe('/small.png');
  });
});

describe('coverFallback', () => {
  it('falls back to the other size', () => {
    expect(coverFallback(pair, '/large.png')).toBe('/small.png');
    expect(coverFallback(pair, '/small.png')).toBe('/large.png');
  });

  it('has nothing to fall back to without a second, different cover', () => {
    expect(coverFallback(coverPair(null, '/large.png'), '/large.png')).toBeNull();
    expect(coverFallback(coverPair('/same.png', '/same.png'), '/same.png')).toBeNull();
    expect(coverFallback(pair, '/other.png')).toBeNull();
    expect(coverFallback(pair, null)).toBeNull();
  });
});
