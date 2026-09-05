// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import MediaTab from './MediaTab.svelte';
import type { MediaItem } from './media';

const items: MediaItem[] = [
  { kind: 'screenshot', url: 'https://s/1.png', caption: 'Game screenshot 1' },
  { kind: 'screenshot', url: 'https://s/2.png', caption: 'Game screenshot 2' },
  { kind: 'youtube', videoId: 'abc', caption: 'Game — trailer' },
];

function html(failed: Record<string, true>) {
  return render(MediaTab, {
    props: { items, failed, onOpen: () => {}, onScreenshotError: () => {} },
  }).body;
}

describe('MediaTab', () => {
  it('renders one tile per item when nothing has failed', () => {
    const body = html({});
    expect(body).toContain('details-media-0');
    expect(body).toContain('details-media-1');
    expect(body).toContain('details-media-2');
  });

  it('drops a failed screenshot tile and leaves the survivors on their own indices', () => {
    const body = html({ 'https://s/1.png': true });
    expect(body).not.toContain('details-media-0');
    expect(body).toContain('details-media-1');
    expect(body).toContain('details-media-2');
  });
});
