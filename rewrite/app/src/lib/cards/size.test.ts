import { describe, expect, it } from 'vitest';
import {
  CARD_SIZES,
  cardMinPx,
  cardSizeLabel,
  columnsOf,
  gridTemplate,
  normalizeCardSize,
} from './size';

describe('normalizeCardSize', () => {
  it('accepts the three stored spellings', () => {
    expect(normalizeCardSize('small')).toBe('small');
    expect(normalizeCardSize('medium')).toBe('medium');
    expect(normalizeCardSize('large')).toBe('large');
  });
  it('trims, because config.toml can be hand-edited', () => {
    expect(normalizeCardSize('  large  ')).toBe('large');
  });
  it('falls back to medium for anything else, case included', () => {
    expect(normalizeCardSize('Large')).toBe('medium');
    expect(normalizeCardSize('')).toBe('medium');
    expect(normalizeCardSize('enormous')).toBe('medium');
  });
});

describe('cardMinPx / gridTemplate', () => {
  it('holds design section 5s three minimum column widths', () => {
    expect(cardMinPx('small')).toBe(120);
    expect(cardMinPx('medium')).toBe(160);
    expect(cardMinPx('large')).toBe(200);
  });
  it('builds the auto-fill template design section 5 specifies', () => {
    expect(gridTemplate('medium')).toBe('repeat(auto-fill, minmax(160px, 1fr))');
  });
  it('labels each size for the toolbar control', () => {
    expect(CARD_SIZES.map(cardSizeLabel)).toEqual(['Small', 'Medium', 'Large']);
  });
});

describe('columnsOf', () => {
  it('is 1 for no element, so keyboard navigation degrades to a list', () => {
    expect(columnsOf(null)).toBe(1);
  });
  it('counts the tracks the browser resolved for an auto-fill grid', () => {
    // getComputedStyle resolves `repeat(auto-fill, ...)` to concrete track
    // sizes, so the column count is the number of space-separated entries.
    const fake = {
      ownerDocument: {
        defaultView: {
          getComputedStyle: () => ({ gridTemplateColumns: '188px 188px 188px 188px' }),
        },
      },
    } as unknown as HTMLElement;
    expect(columnsOf(fake)).toBe(4);
  });
  it('is 1 when the grid has not been laid out yet', () => {
    const fake = {
      ownerDocument: {
        defaultView: { getComputedStyle: () => ({ gridTemplateColumns: 'none' }) },
      },
    } as unknown as HTMLElement;
    expect(columnsOf(fake)).toBe(1);
  });
});

import { CARD_COVER_RATIO, PRIMARY_CENTRE_FRACTION, ACTION_ROW_HEIGHT_PX, TITLE_ROW_HEIGHT_PX } from './size';

describe('card hover geometry (E2E click safety)', () => {
  // Specs click `library-card-<id>` / `game-card-<id>` to open Details, and
  // WebdriverIO clicks an element's CENTRE — which also hovers it, raising
  // the overlay. If the centred Play/Install button or the action row sat
  // under that point, every such click would launch or install instead.
  // The card root is the cover plus a one-line title, so its centre is
  // always slightly BELOW the cover's centre; the overlay keeps that band
  // free.
  const heights = [160, 213, 267, 400]; // small, medium, large, a stretched large

  it('never puts the centred primary action under the card root centre', () => {
    for (const cover of heights) {
      const rootCentre = (cover + TITLE_ROW_HEIGHT_PX) / 2;
      const primaryBottom = cover * PRIMARY_CENTRE_FRACTION + 17;
      expect(primaryBottom).toBeLessThan(rootCentre);
    }
  });

  it('never puts the bottom action row under the card root centre', () => {
    for (const cover of heights) {
      const rootCentre = (cover + TITLE_ROW_HEIGHT_PX) / 2;
      const actionRowTop = cover - ACTION_ROW_HEIGHT_PX;
      expect(actionRowTop).toBeGreaterThan(rootCentre);
    }
  });

  it('states the 3:4 fallback ratio design section 5 requires', () => {
    expect(CARD_COVER_RATIO).toBe('3 / 4');
  });
});
