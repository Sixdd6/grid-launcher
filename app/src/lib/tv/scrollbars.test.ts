import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const css = readFileSync(join(here, 'tv.css'), 'utf-8');

describe('hidden scrollbars (item 7)', () => {
  it('hides the scrollbar on every scroller class the prototype names', () => {
    const sel = ':is(.rail, .rows, .scroll, .list, .page, .split .pane)';
    expect(css).toContain(`${sel} { scrollbar-width: none; }`);
    expect(css).toContain(`${sel}::-webkit-scrollbar { display: none; }`);
    expect(css).toMatch(/\[data-scroll\],\s*\S.*\[data-track\] { scrollbar-width: none; }/);
  });

  it('marks the Server tab panels as scrollers', () => {
    const svelte = readFileSync(join(here, 'tabs/server/TvServer.svelte'), 'utf-8');
    expect(svelte).toContain('<div class="list" data-scroll="list" data-testid="tv-server-none">');
    expect(svelte).toContain('<div class="list" data-scroll="list" data-testid="tv-server-offline">');
  });
});
