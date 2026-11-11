import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { bundledFiles, glyph, GLYPH_BUTTONS, GLYPH_FILES, GLYPH_LABELS, PROMPT_SETS } from './glyphs';

const here = dirname(fileURLToPath(import.meta.url));

describe('glyphs', () => {
  it('matches map.json exactly', () => {
    const map = JSON.parse(readFileSync(join(here, 'map.json'), 'utf-8'));
    expect(GLYPH_FILES).toEqual(map);
  });

  it.each(PROMPT_SETS)('%s has all 13 buttons, each a bundled file with a label', (set) => {
    expect(Object.keys(GLYPH_FILES[set]).sort()).toEqual([...GLYPH_BUTTONS].sort());
    for (const b of GLYPH_BUTTONS) {
      const f = GLYPH_FILES[set][b];
      const files: readonly string[] = typeof f === 'string' ? [f] : f;
      for (const file of files) {
        expect(readdirSync(here)).toContain(file);
        expect(bundledFiles()).toContain(file);
      }
      const g = glyph(set, b);
      expect(g.urls).toHaveLength(files.length);
      for (const u of g.urls) expect(u).not.toBe('');
      expect(g.label.trim()).not.toBe('');
      expect(GLYPH_LABELS[set][b]).toBe(g.label);
    }
  });

  it('ships the license, and no Guide glyph or logo', () => {
    expect(readdirSync(here)).toContain('License.txt');
    expect(GLYPH_BUTTONS).not.toContain('GUIDE' as never);
    expect(readdirSync(here).some((f) => /guide|logo/i.test(f))).toBe(false);
  });

  it('reads the 8BitDo View and Menu as Minus and Plus (ruling 4)', () => {
    expect(glyph('8bitdo', 'VIEW').label).toBe('Minus');
    expect(glyph('8bitdo', 'MENU').label).toBe('Plus');
  });

  it('names the PlayStation face buttons', () => {
    expect(['A', 'B', 'X', 'Y'].map((b) => glyph('ps', b as 'A').label)).toEqual(['Cross', 'Circle', 'Square', 'Triangle']);
  });

  it('draws the keyboard keys GRID binds, with Page Up and Page Down for the right stick', () => {
    expect(glyph('keyboard', 'A').label).toBe('Enter');
    expect(glyph('keyboard', 'B').label).toBe('Esc');
    expect(['LB', 'RB', 'LT', 'RT', 'VIEW', 'MENU'].map((b) => glyph('keyboard', b as 'A').label)).toEqual(['Q', 'E', 'Z', 'R', 'V', 'M']);
    expect(GLYPH_FILES.keyboard.RS).toEqual(['keyboard_page_up.svg', 'keyboard_page_down.svg']);
    expect(glyph('keyboard', 'RS').urls).toHaveLength(2);
  });
});
