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
 * An 11-character YouTube id and nothing else. Nothing here builds a watch
 * URL: an `<iframe>` to `youtube-nocookie.com` cannot play on Linux (the
 * page origin is `tauri://localhost`, a "local scheme" under the W3C
 * referrer policy, so no `Referer` is sent and YouTube answers error 153
 * "Video unavailable" for every embed — tauri-apps/tauri#14422), so the
 * trailer opens in the system browser through `open_youtube_video`, which
 * validates the id again on the Rust side and builds the only URL. This
 * guard decides whether a trailer tile exists at all and gates the
 * thumbnail URL below.
 */
export function isYoutubeId(value: string): boolean {
  return /^[A-Za-z0-9_-]{11}$/.test(value.trim());
}

/**
 * YouTube's static thumbnail CDN. User ruling 2026-09-05: this is the ONE
 * foreign host anything in this app may load, because `/vi/<id>/hqdefault.jpg`
 * needs no API key, no quota and no cookie. It is loaded as a plain `<img>`
 * with `referrerpolicy="no-referrer"` — NEVER through `ensure_image`, which
 * would fetch it via `RommClient` and attach the RomM Authorization header
 * to a request leaving the server's host.
 */
export const YOUTUBE_THUMBNAIL_BASE = 'https://img.youtube.com/vi';

/** The thumbnail URL for `videoId`, or `''` when it is not an 11-character id. */
export function youtubeThumbnailUrl(videoId: string): string {
  const id = videoId.trim();
  if (!isYoutubeId(id)) return '';
  return `${YOUTUBE_THUMBNAIL_BASE}/${id}/hqdefault.jpg`;
}

/** What a trailer/video tile paints behind its play badge. */
export type TilePoster =
  | { kind: 'youtube'; url: string }
  | { kind: 'cover'; url: string | null };

/**
 * The trailer tile's poster. YouTube's thumbnail when there is a real id and
 * it has not already failed to load (offline, or a video with no thumbnail);
 * the game's own server-hosted cover otherwise. `{ kind: 'cover', url: null }`
 * means "no artwork at all" — the tile renders its placeholder, which is
 * still better than the bare play icon this replaces.
 */
export function trailerPoster(
  videoId: string,
  coverUrl: string | null,
  thumbnailFailed: boolean
): TilePoster {
  const thumbnail = youtubeThumbnailUrl(videoId);
  if (thumbnail !== '' && !thumbnailFailed) return { kind: 'youtube', url: thumbnail };
  return { kind: 'cover', url: coverUrl };
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
