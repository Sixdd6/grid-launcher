// @vitest-environment node
/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// Background-contrast ruling (2026-09-05): raise --text-muted in both
// themes, darken light's --danger, and add a theme-flipped --text-halo
// token consumed by the `.over-art` utility class. See
// docs/superpowers/specs/2026-09-05-text-over-art-ruling.md §3.

const css = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

// Extract each theme's token block by its opening selector, matching up to
// its closing brace. None of these blocks nest further braces, so a
// non-greedy match to the next `\n}` is exact.
function block(source: string, pattern: RegExp): string {
  const match = source.match(pattern);
  if (!match) throw new Error(`block not found: ${pattern}`);
  return match[1];
}

const rootBase = block(css, /:root\s*\{([\s\S]*?)\n\}/);
const prefersDark = block(
  css,
  /@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme='light'\]\)\s*\{([\s\S]*?)\n\s*\}\s*\n\}/
);
const dataThemeDark = block(css, /:root\[data-theme='dark'\]\s*\{([\s\S]*?)\n\}/);
const dataThemeLight = block(css, /:root\[data-theme='light'\]\s*\{([\s\S]*?)\n\}/);

describe('changed token values (ruling §3.1)', () => {
  it('raises --text-muted to #3d3d52 in the light base', () => {
    expect(rootBase).toContain('--text-muted: #3d3d52;');
  });

  it('darkens --danger to #c62828 in the light base', () => {
    expect(rootBase).toContain('--danger: #c62828;');
  });

  it('raises --text-muted to #c8c8dc in both dark blocks', () => {
    expect(prefersDark).toContain('--text-muted: #c8c8dc;');
    expect(dataThemeDark).toContain('--text-muted: #c8c8dc;');
  });

  it('restates all three light-theme tokens under [data-theme="light"]', () => {
    expect(dataThemeLight).toContain('--text-muted: #3d3d52;');
    expect(dataThemeLight).toContain('--danger: #c62828;');
  });
});

describe('--text-halo (ruling §3.1)', () => {
  it('is defined in every theme block that defines --text-muted', () => {
    for (const b of [rootBase, prefersDark, dataThemeDark, dataThemeLight]) {
      expect(b).toContain('--text-muted');
      expect(b).toContain('--text-halo:');
    }
  });

  it('uses the theme’s own --bg colour, light value', () => {
    expect(rootBase).toContain('rgba(245, 245, 250, 0.92)');
    expect(rootBase).toContain('rgba(245, 245, 250, 0.6)');
    expect(dataThemeLight).toContain('rgba(245, 245, 250, 0.92)');
    expect(dataThemeLight).toContain('rgba(245, 245, 250, 0.6)');
  });

  it('uses the theme’s own --bg colour, dark value', () => {
    expect(prefersDark).toContain('rgba(7, 7, 15, 0.85)');
    expect(prefersDark).toContain('rgba(7, 7, 15, 0.5)');
    expect(dataThemeDark).toContain('rgba(7, 7, 15, 0.85)');
    expect(dataThemeDark).toContain('rgba(7, 7, 15, 0.5)');
  });
});

describe('.over-art utility (ruling §3.3)', () => {
  it('sets text-shadow: var(--text-halo)', () => {
    const rule = block(css, /(?<!\S)\.over-art\s*\{([\s\S]*?)\n\}/);
    expect(rule.trim()).toBe('text-shadow: var(--text-halo);');
  });

  it('resets text-shadow: none on every opt-out selector', () => {
    const match = css.match(/(\.over-art [^{]+)\{\s*text-shadow: none;\s*\}/);
    expect(match).not.toBeNull();
    const selectorList = match![1];

    const expectedDescendants = [
      'input',
      'select',
      'textarea',
      '.primary',
      '.tag',
      '.actions button',
      '.form-actions button',
      '.row-actions button',
      '.library-banner button',
      '.catalog-row button',
      '.ps3-firmware button',
      '.chip button',
      '.offline button',
      '.update-line button',
      '.browse-secondary',
    ];

    for (const descendant of expectedDescendants) {
      expect(selectorList).toContain(`.over-art ${descendant}`);
    }
  });
});
