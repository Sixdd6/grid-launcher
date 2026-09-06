import { describe, expect, it } from 'vitest';
import {
  OVERVIEW_STRIP_LIMIT,
  galleryItems,
  isYoutubeId,
  nextIndex,
  overviewStrip,
  prevIndex,
  trailerPoster,
  videoMimeType,
  youtubeThumbnailUrl,
  YOUTUBE_THUMBNAIL_BASE,
} from './media';

describe('galleryItems', () => {
  it('lists every screenshot, numbered from one, then the videos', () => {
    expect(
      galleryItems({
        title: 'Super Mario World',
        screenshotUrls: ['http://s/1.png', 'http://s/2.png'],
        youtubeVideoId: 'dQw4w9WgXcQ',
        videoPath: '/assets/romm/resources/roms/101/video.mp4',
      })
    ).toEqual([
      { kind: 'screenshot', url: 'http://s/1.png', caption: 'Super Mario World — screenshot 1' },
      { kind: 'screenshot', url: 'http://s/2.png', caption: 'Super Mario World — screenshot 2' },
      { kind: 'youtube', videoId: 'dQw4w9WgXcQ', caption: 'Super Mario World — trailer' },
      {
        kind: 'video',
        url: '/assets/romm/resources/roms/101/video.mp4',
        caption: 'Super Mario World — video',
      },
    ]);
  });

  it('omits the YouTube tile when the id is not a YouTube id', () => {
    const items = galleryItems({
      title: 'G',
      screenshotUrls: [],
      youtubeVideoId: 'not an id',
      videoPath: '',
    });
    expect(items).toEqual([]);
  });

  it('is empty when the server has no media at all', () => {
    expect(
      galleryItems({ title: 'G', screenshotUrls: [], youtubeVideoId: '', videoPath: '' })
    ).toEqual([]);
  });
});

describe('isYoutubeId', () => {
  it('accepts an 11-character id', () => {
    expect(isYoutubeId('dQw4w9WgXcQ')).toBe(true);
  });

  it('rejects anything else, so no arbitrary string reaches the iframe src', () => {
    expect(isYoutubeId('')).toBe(false);
    expect(isYoutubeId('short')).toBe(false);
    expect(isYoutubeId('../../evil/path')).toBe(false);
    expect(isYoutubeId('dQw4w9WgXcQextra')).toBe(false);
  });
});

describe('viewer navigation', () => {
  it('advances', () => {
    expect(nextIndex(0, 3)).toBe(1);
  });

  it('wraps forward off the end', () => {
    expect(nextIndex(2, 3)).toBe(0);
  });

  it('wraps backward off the start', () => {
    expect(prevIndex(0, 3)).toBe(2);
  });

  it('stays put with a single item', () => {
    expect(nextIndex(0, 1)).toBe(0);
    expect(prevIndex(0, 1)).toBe(0);
  });

  it('never divides by zero on an empty gallery', () => {
    expect(nextIndex(0, 0)).toBe(0);
    expect(prevIndex(0, 0)).toBe(0);
  });
});

describe('overviewStrip', () => {
  it('caps at design §7 first six', () => {
    const urls = Array.from({ length: 9 }, (_, i) => `http://s/${i}.png`);
    expect(overviewStrip(urls)).toHaveLength(OVERVIEW_STRIP_LIMIT);
    expect(overviewStrip(urls)[5]).toBe('http://s/5.png');
  });

  it('passes a shorter list through', () => {
    expect(overviewStrip(['a', 'b'])).toEqual(['a', 'b']);
  });
});

describe('youtubeThumbnailUrl', () => {
  it('builds the static CDN path for a valid id', () => {
    expect(youtubeThumbnailUrl('dQw4w9WgXcQ')).toBe(
      'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg'
    );
  });

  it('trims before building', () => {
    expect(youtubeThumbnailUrl('  dQw4w9WgXcQ  ')).toBe(
      'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg'
    );
  });

  // The id is interpolated into a URL the page loads directly, so anything
  // that is not exactly an id must produce nothing at all.
  it('is blank for anything that is not an 11-character id', () => {
    expect(youtubeThumbnailUrl('')).toBe('');
    expect(youtubeThumbnailUrl('short')).toBe('');
    expect(youtubeThumbnailUrl('https://youtu.be/dQw4w9WgXcQ')).toBe('');
    expect(youtubeThumbnailUrl('../../etc/passwd')).toBe('');
  });

  it('never leaves the one allowed foreign host', () => {
    expect(youtubeThumbnailUrl('dQw4w9WgXcQ').startsWith(`${YOUTUBE_THUMBNAIL_BASE}/`)).toBe(true);
  });
});

describe('trailerPoster', () => {
  it("prefers YouTube's own thumbnail for a valid id", () => {
    expect(trailerPoster('dQw4w9WgXcQ', 'https://romm/cover.png', false)).toEqual({
      kind: 'youtube',
      url: 'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
    });
  });

  it('falls back to the server-hosted cover once the thumbnail has failed', () => {
    expect(trailerPoster('dQw4w9WgXcQ', 'https://romm/cover.png', true)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
  });

  it('falls back to the cover when there is no usable id', () => {
    expect(trailerPoster('', 'https://romm/cover.png', false)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
    expect(trailerPoster('not-an-id', 'https://romm/cover.png', false)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
  });

  it('reports a cover poster with no cover, which the tile renders as its placeholder', () => {
    expect(trailerPoster('', null, false)).toEqual({ kind: 'cover', url: null });
  });
});

describe('videoMimeType', () => {
  it('names the container the extension declares', () => {
    expect(videoMimeType('http://romm/roms/1/101/video/clip.mp4')).toBe('video/mp4');
    expect(videoMimeType('http://romm/roms/1/101/video/clip.webm')).toBe('video/webm');
    expect(videoMimeType('http://romm/roms/1/101/video/clip.mov')).toBe('video/quicktime');
  });

  it('ignores case and any query string, which RomM adds for cache busting', () => {
    expect(videoMimeType('http://romm/clip.WEBM?ts=17')).toBe('video/webm');
    expect(videoMimeType('http://romm/clip.MOV')).toBe('video/quicktime');
  });

  it('falls back to mp4, the container every RomM preview uses', () => {
    expect(videoMimeType('http://romm/clip')).toBe('video/mp4');
    expect(videoMimeType('')).toBe('video/mp4');
    // A directory named ".webm" must not decide the type of a file that has
    // no extension of its own.
    expect(videoMimeType('http://romm/a.webm/clip')).toBe('video/mp4');
  });
});
