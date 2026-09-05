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

function html(failed: Record<string, true>, list: MediaItem[] = items, coverUrl: string | null = null) {
  return render(MediaTab, {
    props: { items: list, failed, coverUrl, onOpen: () => {}, onScreenshotError: () => {} },
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

  it("paints a real trailer with YouTube's own thumbnail", () => {
    const body = html({}, [{ kind: 'youtube', videoId: 'dQw4w9WgXcQ', caption: 'Game — trailer' }]);
    expect(body).toContain('details-media-thumb-0');
    expect(body).toContain('https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg');
    expect(body).toContain('details-media-play-0');
  });

  it('paints the cover instead when the trailer id is not a real id', () => {
    const body = html({}, [{ kind: 'youtube', videoId: 'abc', caption: 'Game — trailer' }], 'https://romm/cover.png');
    expect(body).not.toContain('details-media-thumb-0');
    expect(body).toContain('details-media-poster-0');
    expect(body).not.toContain('img.youtube.com');
  });

  it('gives a hosted video the cover and a play badge', () => {
    const body = html({}, [{ kind: 'video', url: '/v.mp4', caption: 'Game — video' }], 'https://romm/cover.png');
    expect(body).toContain('details-media-poster-0');
    expect(body).toContain('details-media-play-0');
  });
});
