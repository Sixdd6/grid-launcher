import { describe, expect, it } from 'vitest';
import { normalizeTitle, relatedKindLabel, relatedOnServer } from './related';

describe('normalizeTitle', () => {
  it('folds case, trims and collapses whitespace', () => {
    expect(normalizeTitle('  Super   Mario  World ')).toBe('super mario world');
  });

  it('drops a trailing region/tag parenthetical, which server file names carry', () => {
    expect(normalizeTitle('Chrono Trigger (USA)')).toBe('chrono trigger');
  });
});

describe('relatedKindLabel', () => {
  it('names each list', () => {
    expect(relatedKindLabel('similar')).toBe('Similar');
    expect(relatedKindLabel('remake')).toBe('Remake');
    expect(relatedKindLabel('remaster')).toBe('Remaster');
    expect(relatedKindLabel('dlc')).toBe('DLC');
    expect(relatedKindLabel('expansion')).toBe('Expansion');
  });

  it('falls back for a kind a newer backend adds', () => {
    expect(relatedKindLabel('port')).toBe('Related');
  });
});

describe('relatedOnServer', () => {
  const related = [
    { name: 'Super Mario World', kind: 'similar' },
    { name: 'Chrono Trigger', kind: 'remake' },
    { name: 'A Game Nobody Owns', kind: 'similar' },
  ];

  it('keeps only titles the platform list actually holds', () => {
    expect(relatedOnServer(related, ['Super Mario World', 'Chrono Trigger (USA)'])).toEqual([
      { name: 'Super Mario World', kind: 'similar' },
      { name: 'Chrono Trigger', kind: 'remake' },
    ]);
  });

  it('keeps the backend order', () => {
    const out = relatedOnServer(related, ['Chrono Trigger', 'Super Mario World']);
    expect(out.map((r) => r.name)).toEqual(['Super Mario World', 'Chrono Trigger']);
  });

  it('is empty when the platform list has not loaded yet', () => {
    expect(relatedOnServer(related, [])).toEqual([]);
  });

  it('drops a duplicate title the two lists both name', () => {
    const dupes = [
      { name: 'Super Mario World', kind: 'similar' },
      { name: 'super mario world', kind: 'remaster' },
    ];
    expect(relatedOnServer(dupes, ['Super Mario World'])).toEqual([
      { name: 'Super Mario World', kind: 'similar' },
    ]);
  });
});
