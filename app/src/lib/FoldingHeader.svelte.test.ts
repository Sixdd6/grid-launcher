// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import FoldingHeader from './FoldingHeader.svelte';

const title = createRawSnippet(() => ({ render: () => '<h2>PlayStation</h2>' }));
const toolbar = createRawSnippet(() => ({ render: () => '<input data-testid="search" />' }));

describe('FoldingHeader', () => {
  it('renders the title row above the toolbar, unfolded', () => {
    const { body } = render(FoldingHeader, { props: { scroller: null, resetKey: 1, title, toolbar } });
    expect(body.indexOf('<h2>PlayStation</h2>')).toBeLessThan(body.indexOf('data-testid="search"'));
    expect(body).not.toMatch(/class="[^"]*\bfolded\b/);
    expect(body).not.toContain('inert');
  });

  it('folded, hides the title row from focus and assistive tech', () => {
    const { body } = render(FoldingHeader, {
      props: { scroller: null, resetKey: 1, folded: true, title, toolbar },
    });
    expect(body).toMatch(/class="[^"]*\bfolded\b/);
    expect(body).toContain('inert');
  });
});
