// @vitest-environment node
/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// Background-contrast ruling (2026-09-05): raise --text-muted in both
// themes and darken light's --danger. See
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

// Fix round 1: `:root` is the light base (file header comment), so its
// unconditional `--danger: #c62828;` also resolves for dark mode unless
// both dark blocks restate the dark value — CSS custom-property cascade is
// per-declaration, so a block that never mentions `--danger` does not
// override it regardless of specificity. Mirrors how both dark blocks
// already restate --text-muted, --primary, etc.
describe('--danger stays theme-scoped (ruling §3.1/§3.2, fix round 1)', () => {
  it('restates the dark value in both dark blocks', () => {
    expect(prefersDark).toContain('--danger: #ff5050;');
    expect(dataThemeDark).toContain('--danger: #ff5050;');
  });

  it('never resolves to the light value #c62828 in a dark block', () => {
    expect(prefersDark).not.toContain('#c62828');
    expect(dataThemeDark).not.toContain('#c62828');
  });

  it('only the light blocks (base and explicit) declare #c62828', () => {
    expect(rootBase).toContain('--danger: #c62828;');
    expect(dataThemeLight).toContain('--danger: #c62828;');
  });
});

// Look-and-feel plan (2026-10-30): text sits on panels, never on the raw
// art. A panel's opacity is a fixed floor in every theme block, whatever the
// fade, so its text keeps WCAG AA on any art.
describe('panel floor (look-and-feel plan 2026-10-30)', () => {
  it('is 0.92 of the light panel colour in both light blocks', () => {
    expect(rootBase).toContain('--panel: rgba(250, 250, 253, 0.92);');
    expect(dataThemeLight).toContain('--panel: rgba(250, 250, 253, 0.92);');
  });

  it('is 0.92 of the dark panel colour in both dark blocks', () => {
    expect(prefersDark).toContain('--panel: rgba(14, 14, 24, 0.92);');
    expect(dataThemeDark).toContain('--panel: rgba(14, 14, 24, 0.92);');
  });

  it('defines an opaque overlay in every theme block', () => {
    for (const b of [rootBase, prefersDark, dataThemeDark, dataThemeLight]) {
      expect(b).toMatch(/--overlay: #[0-9a-f]{6};/);
    }
  });

  it('gives placeholders the muted text colour at full opacity', () => {
    const rule = block(css, /::placeholder\s*\{([\s\S]*?)\n\}/);
    expect(rule).toContain('color: var(--text-muted);');
    expect(rule).toContain('opacity: 1;');
  });
});

describe('.icon-btn hover/focus colour (user ruling (b))', () => {
  it('turns the background to --surface and the glyph to --primary', () => {
    const rule = block(css, /\.icon-btn:hover,\s*\n\.icon-btn:focus-visible\s*\{([\s\S]*?)\n\}/);
    expect(rule).toContain('background: var(--surface);');
    expect(rule).toContain('color: var(--primary);');
  });
});
