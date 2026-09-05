// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Image from './Image.svelte';

// `svelte/server` never runs `$effect`, so the fetch that moves `status`
// from `loading` to `ready`/`error` cannot fire here: SSR always shows the
// loading skeleton, even for a null `url`. That is the one branch reachable
// without a DOM harness, and it pins the contract that a fresh Image never
// renders the "failed" placeholder text before it has tried to load.
describe('Image', () => {
  it('renders the loading skeleton, not the placeholder text, before any fetch', () => {
    const { body } = render(Image, {
      props: { url: 'https://romm/cover.png', alt: 'Cover', placeholder: 'No cover' },
    });
    expect(body).toContain('class="skeleton ');
    expect(body).toContain('aria-hidden="true"');
    expect(body).not.toContain('No cover');
    expect(body).not.toContain('<img');
  });
});
