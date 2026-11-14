import { describe, expect, it } from 'vitest';
import {
  normalizeTvCardLabels,
  normalizeTvCardSize,
  TV_CARD_LABEL_MODES,
  TV_CARD_SIZES,
  TV_LABEL_ROW_REM,
  tvCardLabelsLabel,
  tvCardSizeLabel,
  tvCoverRem,
  tvLabelRowRem,
} from './gridSettings';

describe('TV card size', () => {
  it('is Small 8.5rem, Medium 10rem, Large 12.5rem', () => {
    expect(TV_CARD_SIZES.map(tvCoverRem)).toEqual([8.5, 10, 12.5]);
    expect(TV_CARD_SIZES.map(tvCardSizeLabel)).toEqual(['Small', 'Medium', 'Large']);
  });

  it('normalizes anything else to medium, as the Rust side does', () => {
    expect(normalizeTvCardSize('small')).toBe('small');
    expect(normalizeTvCardSize(' large ')).toBe('large');
    expect(normalizeTvCardSize('medium')).toBe('medium');
    expect(normalizeTvCardSize('Large')).toBe('medium');
    expect(normalizeTvCardSize('huge')).toBe('medium');
    expect(normalizeTvCardSize('')).toBe('medium');
  });
});

describe('TV game labels', () => {
  it('reads Always visible and Visible on focus', () => {
    expect(TV_CARD_LABEL_MODES.map(tvCardLabelsLabel)).toEqual(['Always visible', 'Visible on focus']);
  });

  it('normalizes anything else to always, the desktop hover included', () => {
    expect(normalizeTvCardLabels('focus')).toBe('focus');
    expect(normalizeTvCardLabels(' focus ')).toBe('focus');
    expect(normalizeTvCardLabels('always')).toBe('always');
    expect(normalizeTvCardLabels('hover')).toBe('always');
    expect(normalizeTvCardLabels('Focus')).toBe('always');
    expect(normalizeTvCardLabels('')).toBe('always');
  });

  it('reserves a label row only while every title shows', () => {
    expect(tvLabelRowRem('always')).toBeCloseTo(1.76);
    expect(tvLabelRowRem('always')).toBe(TV_LABEL_ROW_REM);
    expect(tvLabelRowRem('focus')).toBe(0);
  });
});
