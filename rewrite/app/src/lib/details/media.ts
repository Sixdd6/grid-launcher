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

/**
 * The gallery items a user can actually look at: every screenshot whose
 * image failed to load is dropped, exactly as `MediaTab.svelte` drops its
 * tile. A trailer or a hosted video is never dropped — `failed` holds only
 * screenshot URLs (`onScreenshotError`), and those two kinds report their
 * own failures inside the viewer.
 */
export function viewableItems(items: MediaItem[], failed: Record<string, true>): MediaItem[] {
  return items.filter((item) => !(item.kind === 'screenshot' && failed[item.url] === true));
}

/**
 * Where `items[index]` sits in `viewableItems(items, failed)`.
 *
 * When that item is itself a failed screenshot there is no position for it,
 * so this answers with the NEXT viewable item after it, wrapping past the
 * end — the viewer then moves on by itself instead of showing a dead frame.
 * `null` means nothing is viewable at all (or `index` is outside `items`),
 * and the caller must not open — or must close — the viewer.
 */
export function viewableIndex(
  items: MediaItem[],
  failed: Record<string, true>,
  index: number
): number | null {
  if (index < 0 || index >= items.length) return null;
  const viewable = viewableItems(items, failed);
  if (viewable.length === 0) return null;
  // Walk forward from `index` (wrapping) to the first item that survived the
  // filter, then report that item's position in the filtered list.
  for (let step = 0; step < items.length; step += 1) {
    const candidate = items[(index + step) % items.length];
    const position = viewable.indexOf(candidate);
    if (position !== -1) return position;
  }
  return null;
}

/**
 * The inverse of `viewableIndex`: where `viewableItems(items, failed)[position]`
 * sits in `items`.
 *
 * `-1` when `position` is outside the viewable list. The viewer only ever
 * reports a position it has just rendered, so that answer means the list
 * emptied under it and the caller must close.
 */
export function fullIndex(
  items: MediaItem[],
  failed: Record<string, true>,
  position: number
): number {
  const viewable = viewableItems(items, failed);
  if (position < 0 || position >= viewable.length) return -1;
  return items.indexOf(viewable[position]);
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

/** The line the viewer shows when the bytes of a hosted video never arrive. */
export const VIDEO_LOAD_FAILED = 'This video could not be loaded';

/**
 * The whole user-facing line for a failed video read. The generic sentence
 * always leads, so the user is never shown a bare backend string as if it
 * were the message; when the backend supplied a reason it follows in
 * parentheses, verbatim apart from trimming and one redundant full stop.
 *
 * Distinct on purpose from the decode-failure line ("This video could not be
 * played"): that one means the bytes DID arrive.
 */
export function videoLoadMessage(detail: string | null): string {
  const trimmed = (detail ?? '').trim().replace(/\.$/, '').trim();
  if (trimmed === '') return VIDEO_LOAD_FAILED;
  return `${VIDEO_LOAD_FAILED} (${trimmed}).`;
}

/**
 * The MIME type a hosted video's `Blob` is built with, read from the URL's
 * own extension. The bytes reach the page over IPC with no headers, so the
 * `<video>` element has nothing else to go on; a wrong type stops WebKitGTK
 * from decoding the file at all.
 *
 * A query string is dropped first (RomM appends cache-busting parameters),
 * and only the last path segment is examined, so a directory called
 * `video.webm` never decides the type of the file inside it. Anything
 * unrecognised is `video/mp4` — the container every RomM `path_video`
 * preview uses.
 */
export function videoMimeType(url: string): string {
  const path = url.split('?')[0].split('#')[0];
  const name = path.slice(path.lastIndexOf('/') + 1).toLowerCase();
  if (name.endsWith('.webm')) return 'video/webm';
  if (name.endsWith('.mov')) return 'video/quicktime';
  return 'video/mp4';
}
