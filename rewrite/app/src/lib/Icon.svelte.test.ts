// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Icon from './Icon.svelte';

describe('Icon', () => {
  it('is hidden from the accessibility tree when unlabelled', () => {
    const { body } = render(Icon, { props: { name: 'close' } });
    expect(body).toContain('aria-hidden="true"');
    expect(body).not.toContain('role="img"');
  });

  it('exposes role="img" and the name when labelled', () => {
    const { body } = render(Icon, { props: { name: 'close', label: 'Close' } });
    expect(body).toContain('role="img"');
    expect(body).toContain('aria-label="Close"');
    expect(body).not.toContain('aria-hidden');
  });

  it('treats an empty label the same as no label', () => {
    const { body } = render(Icon, { props: { name: 'close', label: '' } });
    expect(body).toContain('aria-hidden="true"');
    expect(body).not.toContain('role="img"');
  });

  it.each(['star', 'play'] as const)('%s is a filled mark', (name) => {
    const { body } = render(Icon, { props: { name } });
    const path = body.match(/<path[^>]*>/)?.[0] ?? '';
    expect(path).toContain('fill="currentColor"');
    expect(path).toContain('stroke="none"');
  });

  it('does not fill an outline icon', () => {
    // Outline icons paint with stroke only; `fill="currentColor"` is
    // reserved for the solid marks (`star`, `play`).
    const { body } = render(Icon, { props: { name: 'close' } });
    expect(body).not.toContain('fill="currentColor"');
  });

  it('renders the requested size on width and height', () => {
    const { body } = render(Icon, { props: { name: 'close', size: 20 } });
    expect(body).toContain('width="20"');
    expect(body).toContain('height="20"');
  });

  it('always uses the 24x24 viewBox', () => {
    const { body } = render(Icon, { props: { name: 'close' } });
    expect(body).toContain('viewBox="0 0 24 24"');
  });
});
