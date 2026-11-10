import { describe, expect, it } from 'vitest';
import { choiceLabel, rowA, sliderPercent, sliderStep, textValue } from './rows';

describe('rows', () => {
  it('names the A action per kind (item 129)', () => {
    const base = { key: 'k', label: 'L' };
    expect(rowA({ ...base, type: 'switch', on: true, set: () => {} })).toBe('Switch');
    expect(rowA({ ...base, type: 'text', value: '', edit: () => {} })).toBe('Edit');
    expect(rowA({ ...base, type: 'slider', value: 0, min: 0, max: 1, step: 1, format: String, set: () => {} })).toBe('Select');
    expect(rowA({ ...base, type: 'choice', value: '', options: [], choose: () => {} })).toBe('Select');
    expect(rowA({ ...base, type: 'action', run: () => {} })).toBe('Select');
    expect(rowA({ ...base, type: 'info', value: '' })).toBeNull();
  });
  it('slider percent and clamped steps', () => {
    expect(sliderPercent({ value: 45, min: 0, max: 100 })).toBe(45);
    expect(sliderStep({ value: 95, min: 0, max: 100, step: 10 }, 1)).toBe(100);
    expect(sliderStep({ value: 5, min: 0, max: 100, step: 10 }, -1)).toBe(0);
  });
  it('choice and text values', () => {
    expect(choiceLabel({ value: 'b', options: [['a', 'A'], ['b', 'B']] })).toBe('B');
    expect(textValue({ value: '', mask: false })).toEqual({ text: 'Not set', unset: true });
    expect(textValue({ value: 'secret', mask: true })).toEqual({ text: '••••••••', unset: false });
    expect(textValue({ value: 'sam', mask: false })).toEqual({ text: 'sam', unset: false });
  });
});
