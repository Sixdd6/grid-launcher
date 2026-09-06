// @vitest-environment node
import { readFileSync } from 'node:fs';
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

// The `<img>` tags only exist once `$effect` has resolved the cache path,
// and SSR never runs an effect (see above), so their attributes cannot be
// asserted on rendered markup — there is no `<img>` in it. The component's
// SOURCE is read instead: it is the only place these attributes are written,
// and a change to them cannot slip past this.
describe('the loading attributes on Image.svelte’s <img> tags', () => {
  const source = readFileSync(new URL('./Image.svelte', import.meta.url), 'utf8');
  // Whitespace after the name, so the `<img>` in a doc comment above is
  // not mistaken for a tag.
  const tags = source.match(/<img\s[^>]*>/g) ?? [];

  it('covers both the cover and the blurred backdrop copy', () => {
    expect(tags).toHaveLength(2);
  });

  it('loads eagerly and decodes asynchronously', () => {
    for (const tag of tags) {
      expect(tag).toContain('loading="eager"');
      expect(tag).toContain('decoding="async"');
      expect(tag).not.toContain('loading="lazy"');
    }
  });
});
