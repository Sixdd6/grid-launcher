import { describe, expect, it } from 'vitest';
import {
  ACTION_BAR_HEIGHT_PX,
  CARD_SIZES,
  LABEL_GAP_PX,
  LABEL_HEIGHT_PX,
  cardMinPx,
  cardSizeLabel,
  labelRowPx,
  normalizeCardSize,
  targetRowPx,
} from './size';
import { MIN_COVER_PX } from './layout';

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

describe('card sizes', () => {
  it('holds design section 5s three minimum widths', () => {
    expect(cardMinPx('small')).toBe(120);
    expect(cardMinPx('medium')).toBe(160);
    expect(cardMinPx('large')).toBe(200);
  });
  it('gives Equal height the 3:4 height of each width', () => {
    expect(CARD_SIZES.map(targetRowPx)).toEqual([160, 213, 267]);
  });
  it('labels each size for the toolbar control', () => {
    expect(CARD_SIZES.map(cardSizeLabel)).toEqual(['Small', 'Medium', 'Large']);
  });
});

describe('card hover geometry (E2E click safety)', () => {
  // Specs click `library-card-<id>` / `game-card-<id>` to open Details, and
  // WebdriverIO clicks an element's CENTRE, which also hovers it and shows
  // the hover bar. If a bar control sat under that point, every such click
  // would launch or install instead. The card root is the cover plus the
  // label row, and the bar is the cover's bottom strip; the layout never
  // makes a cover shorter than MIN_COVER_PX (`layout.test.ts`), so checking
  // that height covers every shape, size and layout.
  const rootCentre = (cover: number, labelRow: number) => (cover + labelRow) / 2;

  it('never puts the hover bar under the card root centre, in either label mode', () => {
    for (const cover of [MIN_COVER_PX, 160, 213, 400]) {
      for (const labels of ['always', 'hover'] as const) {
        expect(cover - ACTION_BAR_HEIGHT_PX).toBeGreaterThan(rootCentre(cover, labelRowPx(labels)));
      }
    }
  });

  it('reserves a label row for "Always visible" and none for "Visible on hover"', () => {
    expect(labelRowPx('always')).toBe(LABEL_GAP_PX + LABEL_HEIGHT_PX);
    expect(labelRowPx('hover')).toBe(0);
  });
});
