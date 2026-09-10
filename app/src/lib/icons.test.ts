import { describe, expect, it } from 'vitest';
import { FILLED_ICONS, ICONS, type IconName } from './icons';

// The nine names the UI asks for. Written out here rather than derived from
// `ICONS` so that deleting an icon a call site still uses fails this test
// instead of silently shrinking the set.
const EXPECTED: IconName[] = [
  'close',
  'chevronLeft',
  'chevronRight',
  'arrowLeft',
  'cloud',
  'star',
  'download',
  'play',
  'grid',
];

// Every SVG path command letter, plus the number/separator characters a
// coordinate can use. Anything else (a colour, a `<`, a stray identifier)
// means the entry is not path data.
const PATH_CHARS = /^M[MmLlHhVvCcSsQqTtAaZz0-9 ,.-]*$/;

describe('ICONS', () => {
  it('has exactly the names the UI asks for', () => {
    expect(Object.keys(ICONS).sort()).toEqual([...EXPECTED].sort());
  });

  it.each(EXPECTED)('%s is a non-empty path string starting with a moveto', (name) => {
    const d = ICONS[name];
    expect(typeof d).toBe('string');
    expect(d.length).toBeGreaterThan(0);
    expect(d.startsWith('M')).toBe(true);
  });

  it.each(EXPECTED)('%s uses only SVG path commands and coordinates', (name) => {
    expect(ICONS[name]).toMatch(PATH_CHARS);
  });

  it.each(EXPECTED)('%s carries at least two drawing commands', (name) => {
    // A single moveto draws nothing. Every icon in the set is a real shape.
    const commands = ICONS[name].match(/[MmLlHhVvCcSsQqTtAaZz]/g) ?? [];
    expect(commands.length).toBeGreaterThanOrEqual(2);
  });

  it.each(EXPECTED)('%s has no scientific notation or NaN', (name) => {
    expect(ICONS[name]).not.toMatch(/e[+-]?\d/i);
    expect(ICONS[name]).not.toContain('NaN');
  });
});

describe('FILLED_ICONS', () => {
  it('is the two solid marks', () => {
    expect([...FILLED_ICONS].sort()).toEqual(['play', 'star']);
  });

  it('only names icons that exist', () => {
    for (const name of FILLED_ICONS) expect(ICONS[name]).toBeDefined();
  });
});
