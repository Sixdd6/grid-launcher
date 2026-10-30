import { describe, expect, it } from 'vitest';
import {
  ACTION_BAR_HEIGHT_PX,
  CARD_CELL_ASPECT,
  CARD_SIZES,
  LABEL_GAP_PX,
  LABEL_HEIGHT_PX,
  cardMinPx,
  labelRowPx,
  cardSizeLabel,
  columnsOf,
  gridTemplate,
  parseTracks,
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
  it('is 1 for a grid in a hidden view, whose template never resolved', () => {
    // A `hidden` view has no layout, so the computed value is the specified
    // template: split on spaces it would read as three tracks.
    const fake = {
      ownerDocument: {
        defaultView: { getComputedStyle: () => ({ gridTemplateColumns: gridTemplate('medium') }) },
      },
    } as unknown as HTMLElement;
    expect(columnsOf(fake)).toBe(1);
  });
});

describe('parseTracks', () => {
  it('reads resolved px tracks, fractions included', () => {
    expect(parseTracks('188.25px 188.25px 188.25px')).toEqual([188.25, 188.25, 188.25]);
  });
  it('is null for no layout, a hidden grid and an unresolved template', () => {
    expect(parseTracks('')).toBe(null);
    expect(parseTracks(undefined)).toBe(null);
    expect(parseTracks('none')).toBe(null);
    expect(parseTracks(gridTemplate('small'))).toBe(null);
    expect(parseTracks('0px')).toBe(null);
  });
});

describe('card hover geometry (E2E click safety)', () => {
  // Specs click `library-card-<id>` / `game-card-<id>` to open Details, and
  // WebdriverIO clicks an element's CENTRE — which also hovers it, showing
  // the hover bar. If a bar control sat under that point, every such click
  // would launch or install instead. The card root is the 3:4 cell plus the
  // label row; the cover sits on the cell's bottom edge whatever its shape,
  // so the bar (the cover's bottom strip) is always at the cell's bottom.
  //
  // Every number here is an exported constant that `GameCard.svelte` feeds
  // into its own CSS as a custom property, so this arithmetic guards what
  // the browser actually lays out.
  const cells = [160, 213, 267, 400]; // small, medium, large, a stretched large
  const rootCentre = (cell: number, labelRow: number) => (cell + labelRow) / 2;

  it('never puts the hover bar under the card root centre, in either label mode', () => {
    for (const cell of cells) {
      for (const labels of ['always', 'hover'] as const) {
        const barTop = cell - ACTION_BAR_HEIGHT_PX;
        expect(barTop).toBeGreaterThan(rootCentre(cell, labelRowPx(labels)));
      }
    }
  });

  it('derives the cells from the grids own minimum column widths', () => {
    // 3:4 at each minimum column width — the smallest cell each size can
    // lay out, and so the tightest case for the bound above.
    const derived = CARD_SIZES.map((size) => Math.round(cardMinPx(size) / CARD_CELL_ASPECT));
    expect(derived).toEqual([160, 213, 267]);
    for (const cell of derived) expect(cells).toContain(cell);
  });

  it('reserves a label row for "Always visible" and none for "Visible on hover"', () => {
    expect(labelRowPx('always')).toBe(LABEL_GAP_PX + LABEL_HEIGHT_PX);
    expect(labelRowPx('hover')).toBe(0);
  });
});
