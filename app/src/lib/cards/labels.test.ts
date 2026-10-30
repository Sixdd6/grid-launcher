import { describe, expect, it } from 'vitest';
import { CARD_LABEL_MODES, cardLabelsLabel, normalizeCardLabels } from './labels';

describe('game labels setting', () => {
  it('reads hover, and anything else as always (the Rust normaliser)', () => {
    expect(normalizeCardLabels('hover')).toBe('hover');
    expect(normalizeCardLabels('  hover ')).toBe('hover');
    expect(normalizeCardLabels('always')).toBe('always');
    expect(normalizeCardLabels('Hover')).toBe('always');
    expect(normalizeCardLabels('')).toBe('always');
  });

  it('names both modes for the select', () => {
    expect(CARD_LABEL_MODES.map(cardLabelsLabel)).toEqual(['Always visible', 'Visible on hover']);
  });
});
