// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';

import CardGrid from './CardGrid.svelte';
import { INITIAL_WINDOW_CARDS } from './cards/layout';

// `svelte/server` never runs `$effect`, so the grid is never measured here:
// what renders is the window before the first measurement. The scroll-driven
// window and the layouts are `cards/layout.ts`'s, and are tested there.
type Item = { id: number };

const card = createRawSnippet((item: () => Item, index: () => number) => ({
  render: () => `<div class="c" data-id="${item().id}" data-index="${index()}"></div>`,
}));

function grid(count: number): string {
  const items = Array.from({ length: count }, (_, i) => ({ id: 100 + i }));
  return render(CardGrid<Item>, {
    props: { items, key: (item: Item) => item.id, cover: () => null, card, size: 'medium', labels: 'always', display: 'rows', gridId: 'test-grid' },
  }).body;
}

const cards = (body: string) => body.match(/class="c"/g)?.length ?? 0;

describe('CardGrid', () => {
  it('renders a short list whole, with only its own padding', () => {
    const body = grid(3);
    expect(cards(body)).toBe(3);
    expect(body).toContain('--pad-top: 18px');
    expect(body).toContain('--pad-bottom: 24px');
    expect(body).toContain('data-display="rows"');
    expect(body).toContain('data-index="2"');
  });

  it('renders the first INITIAL_WINDOW_CARDS of a long list before measuring', () => {
    const body = grid(2500);
    expect(cards(body)).toBe(INITIAL_WINDOW_CARDS);
    expect(body).toContain(`data-id="${100 + INITIAL_WINDOW_CARDS - 1}"`);
    expect(body).not.toContain(`data-id="${100 + INITIAL_WINDOW_CARDS}"`);
  });

  it('puts nothing in the grid but the cards', () => {
    // `launch.spec.ts` opens the first card with `library-grid > *:first-child`.
    const body = grid(3);
    const inner = body.slice(body.indexOf('data-testid="test-grid"'));
    expect(inner.replace(/^[^>]*>/, '').trimStart()).toMatch(/^(<!--[^>]*-->)*<div class="c"/);
  });
});
