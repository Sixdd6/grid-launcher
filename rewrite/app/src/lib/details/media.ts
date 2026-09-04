// The Media tab's gallery and the fullscreen viewer's navigation (design
// §7). Pure: `MediaTab.svelte` renders these items and `MediaViewer.svelte`
// walks them, but neither decides what is in the list or what comes next.

export type MediaItem =
  | { kind: 'screenshot'; url: string; caption: string }
  | { kind: 'youtube'; videoId: string; caption: string }
  | { kind: 'video'; url: string; caption: string };

export type MediaGalleryInput = {
  title: string;
  /** Already resolved + host-filtered absolute URLs (`RomDetail.screenshot_urls`). */
  screenshotUrls: string[];
  /** `RomDetail.youtube_video_id`. */
  youtubeVideoId: string;
  /** `RomDetail.video_path`, server-relative and NOT yet cached. */
  videoPath: string;
};

/**
 * An 11-character YouTube id and nothing else. The id is interpolated into
 * an iframe `src`, so anything that is not exactly an id — a path, a full
 * URL, an empty string — must not reach it.
 */
export function isYoutubeId(value: string): boolean {
  return /^[A-Za-z0-9_-]{11}$/.test(value.trim());
}

/** The privacy-preserving embed host; the only frame origin the CSP allows. */
export function youtubeEmbedUrl(videoId: string): string {
  return `https://www.youtube-nocookie.com/embed/${videoId}`;
}

/** Screenshots first (source order), then the trailer, then a hosted video. */
export function galleryItems(input: MediaGalleryInput): MediaItem[] {
  const items: MediaItem[] = input.screenshotUrls.map((url, i) => ({
    kind: 'screenshot' as const,
    url,
    caption: `${input.title} — screenshot ${i + 1}`,
  }));
  const videoId = input.youtubeVideoId.trim();
  if (isYoutubeId(videoId)) {
    items.push({ kind: 'youtube', videoId, caption: `${input.title} — trailer` });
  }
  const videoPath = input.videoPath.trim();
  if (videoPath !== '') {
    items.push({ kind: 'video', url: videoPath, caption: `${input.title} — video` });
  }
  return items;
}

/** The next item, wrapping. `0` for an empty gallery — never `NaN`. */
export function nextIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (current + 1) % count;
}

/** The previous item, wrapping. `0` for an empty gallery. */
export function prevIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (current - 1 + count) % count;
}

/** Design §7 Overview: "screenshot strip (first six of `merged_screenshots`)". */
export const OVERVIEW_STRIP_LIMIT = 6;

export function overviewStrip(urls: string[]): string[] {
  return urls.slice(0, OVERVIEW_STRIP_LIMIT);
}
