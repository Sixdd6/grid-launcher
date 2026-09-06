<script lang="ts">
  import { api } from '../api';
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import { nextIndex, prevIndex, trailerPoster, videoMimeType, type MediaItem } from './media';

  let {
    items,
    index,
    onIndex,
    onClose,
    failed,
    onScreenshotError,
    coverUrl,
  }: {
    items: MediaItem[];
    index: number;
    onIndex: (index: number) => void;
    onClose: () => void;
    failed: Record<string, true>;
    onScreenshotError: (url: string) => void;
    /** The game's large cover — the trailer poster's fallback once YouTube's
     *  thumbnail is unreachable or has already failed (mirrors MediaTab). */
    coverUrl: string | null;
  } = $props();

  let viewerEl = $state<HTMLElement | null>(null);
  let current = $derived(items[index] ?? null);

  // A server-hosted video is fetched through the session client, cached by
  // the backend, and handed to the page as raw bytes over IPC. The server
  // URL never reaches the DOM, so no request from the page needs a token.
  //
  // Bytes rather than a file URL because WebKitGTK 2.52 (the Linux webview)
  // will not decode media served from a custom URI scheme: `asset:` answers
  // correct 206 Range requests and the element still fails with
  // MEDIA_ERR_SRC_NOT_SUPPORTED. A `blob:` object URL built in the page
  // plays. `videoSrc` is therefore an object URL, revoked on every teardown.
  let videoSrc = $state<string | null>(null);
  /** The element could not decode the bytes it was given. */
  let videoError = $state(false);
  /** The backend refused to supply the bytes — its message, shown verbatim
   *  (e.g. the 64 MiB cap's "video too large to play in-app"). */
  let videoLoadError = $state<string | null>(null);

  // A trailer never plays in-app (Task 5): the button opens the system
  // browser through a validated Tauri command, and any failure to launch
  // it (no browser configured, the opener plugin erroring) surfaces here.
  let youtubeError = $state<string | null>(null);

  // Which YouTube thumbnails have failed, keyed by video id — same shape as
  // `MediaTab.svelte`'s guard, kept local because this popup and the tab
  // gallery are independent DOM trees with their own <img> elements.
  let thumbnailFailed = $state<Record<string, true>>({});
  function markThumbnailFailed(videoId: string) {
    thumbnailFailed = { ...thumbnailFailed, [videoId]: true };
  }

  $effect(() => {
    const item = current;
    videoSrc = null;
    videoError = false;
    videoLoadError = null;
    youtubeError = null;
    if (item === null || item.kind !== 'video') return;
    let cancelled = false;
    // Created only on a resolve that is still wanted, so a read that lands
    // after the user moved on never leaves an object URL behind.
    let objectUrl: string | null = null;
    api
      .readVideo(item.url)
      .then((bytes) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: videoMimeType(item.url) }));
        videoSrc = objectUrl;
      })
      .catch((e) => {
        if (cancelled) return;
        // Three lines, never interchangeable. This one means the bytes never
        // arrived at all. The backend's own text is shown verbatim when it
        // has one (the 64 MiB cap says "video too large to play in-app"); a
        // rejection carrying no message falls back to the generic line.
        const message = e instanceof Error ? e.message : String(e);
        videoLoadError = message.trim() === '' ? 'This video could not be loaded' : message;
      });
    return () => {
      cancelled = true;
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
    };
  });

  $effect(() => {
    viewerEl?.focus();
  });

  function go(next: number) {
    onIndex(next);
  }

  function onKey(e: KeyboardEvent) {
    // The popup behind this also closes on Escape; without stopping the
    // event, one press would shut both and the user would lose their place.
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      go(nextIndex(index, items.length));
      return;
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      go(prevIndex(index, items.length));
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

{#if current}
  <div
    data-testid="media-viewer"
    class="viewer"
    bind:this={viewerEl}
    role="dialog"
    aria-modal="true"
    aria-label={current.caption}
    tabindex="-1"
    onkeydown={onKey}
    onclick={onBackdropClick}
  >
    <button data-testid="media-viewer-close" class="icon-btn icon close" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>

    {#if items.length > 1}
      <button
        data-testid="media-viewer-prev"
        class="icon-btn icon prev"
        onclick={() => go(prevIndex(index, items.length))}
        aria-label="Previous"
      >
        <Icon name="chevronLeft" size={20} />
      </button>
      <button
        data-testid="media-viewer-next"
        class="icon-btn icon next"
        onclick={() => go(nextIndex(index, items.length))}
        aria-label="Next"
      >
        <Icon name="chevronRight" size={20} />
      </button>
    {/if}

    <div class="stage">
      {#if current.kind === 'screenshot' && failed[current.url]}
        <!-- User ruling 2026-09-05: the viewer does NOT auto-advance past a
             dead screenshot. Dropping the item would shift every index under
             the user, and advancing would loop forever when every item
             fails; an explicit line is the honest answer. The tile itself is
             already gone from the Media tab behind this. -->
        <p class="pending" data-testid="media-viewer-image-error">
          This screenshot could not be loaded
        </p>
      {:else if current.kind === 'screenshot'}
        <Image
          url={current.url}
          alt={current.caption}
          placeholder="Screenshot"
          data-testid="media-viewer-image"
          onerror={() => onScreenshotError(current.url)}
        />
      {:else if current.kind === 'youtube'}
        <!-- No embed: an iframe to youtube-nocookie.com never plays on
             Linux. The page origin is `tauri://localhost`, a "local scheme"
             under the W3C referrer policy, so no `Referer` header is ever
             sent and YouTube answers error 153 ("Video unavailable") for
             every embed (tauri-apps/tauri#14422) — no markup fix works
             around it. The poster opens the trailer in the system browser
             instead. -->
        {@const poster = trailerPoster(
          current.videoId,
          coverUrl,
          thumbnailFailed[current.videoId] === true
        )}
        <div class="youtube-wrap">
          <div class="frame youtube-frame">
            {#if poster.kind === 'youtube'}
              <!-- Plain <img>, deliberately NOT `Image.svelte`: that
                   component fetches through `ensure_image` -> RommClient,
                   which would attach the RomM token to a foreign host.
                   `no-referrer` keeps this app's URL out of the request
                   entirely (same reasoning as MediaTab.svelte's tile
                   poster). -->
              <img
                src={poster.url}
                alt=""
                aria-hidden="true"
                referrerpolicy="no-referrer"
                draggable="false"
                onerror={() => markThumbnailFailed(current.videoId)}
                onload={(e) => {
                  // YouTube answers a missing thumbnail with HTTP 200 and a
                  // grey 120×90 placeholder, so `onerror` never fires for it.
                  if ((e.currentTarget as HTMLImageElement).naturalWidth <= 120)
                    markThumbnailFailed(current.videoId);
                }}
              />
            {:else}
              <Image url={poster.url} alt="" placeholder="Trailer" />
            {/if}
            <button
              class="watch-btn"
              data-testid="media-viewer-youtube-open"
              onclick={() => {
                youtubeError = null;
                api
                  .openYoutubeVideo(current.videoId)
                  .catch((e) => (youtubeError = e instanceof Error ? e.message : String(e)));
              }}
            >
              Watch on YouTube
            </button>
          </div>
          <p class="youtube-note" data-testid="media-viewer-youtube-note">
            Trailers open in your browser.
          </p>
          {#if youtubeError}
            <p class="youtube-error" data-testid="media-viewer-youtube-error">{youtubeError}</p>
          {/if}
        </div>
      {:else if videoSrc}
        <div class="video-wrap">
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            data-testid="media-viewer-video"
            class="frame"
            src={videoSrc}
            controls
            onerror={() => (videoError = true)}
          ></video>
          {#if videoError}
            <!-- The backend fetched and cached the file (`read_video`
                 already gated it on Content-Type and its `ftyp` magic) and
                 the bytes reached the page, but the element could not decode
                 them — a bad codec, or a file truncated past the header
                 check. One of three distinct lines: this one means the bytes
                 arrived, so the element stays on screen. -->
            <p class="pending" data-testid="media-viewer-video-error">
              This video could not be played
            </p>
          {/if}
        </div>
      {:else if videoLoadError !== null}
        <!-- The backend refused or failed to supply the bytes. Its own text
             is shown verbatim; it names no path, URL or credential by
             construction (`read_cached_video` in commands.rs asserts that). -->
        <p class="pending" data-testid="media-viewer-video-load-error">{videoLoadError}</p>
      {:else}
        <p class="pending" data-testid="media-viewer-video-pending">Loading video…</p>
      {/if}
    </div>

    <p class="caption" data-testid="media-viewer-caption">{current.caption}</p>
  </div>
{/if}

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(0, 0, 0, 0.9);
    display: grid;
    grid-template-rows: 1fr auto;
    place-items: center;
    padding: 48px;
    box-sizing: border-box;
  }

  .stage {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .stage :global(img) {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .frame {
    width: min(100%, 1280px);
    aspect-ratio: 16 / 9;
    border: none;
    background: #000;
  }

  .youtube-wrap,
  .video-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
  }

  .video-wrap .pending {
    margin-top: 12px;
  }

  .youtube-frame {
    position: relative;
    overflow: hidden;
  }

  /* Covers both the plain thumbnail <img> and the one `Image.svelte`
     renders once its cover fallback is ready — same object-fit-cover
     treatment `.video-tile .poster` gives the Media tab's tile. */
  .youtube-frame :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .watch-btn {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font: inherit;
    font-weight: 600;
    padding: 12px 24px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .watch-btn:hover,
  .watch-btn:focus-visible {
    background: var(--primary-hover);
  }

  /* On the viewer's always-dark scrim, same as `.caption`/`.pending` above. */
  .youtube-note {
    margin: 12px 0 0;
    color: #fff;
    font-size: 13px;
  }

  .youtube-error {
    margin: 6px 0 0;
    color: var(--danger);
    font-size: 13px;
  }

  .caption {
    margin: 12px 0 0;
    color: #fff;
    font-size: 13px;
    text-align: center;
  }

  .pending {
    margin: 0;
    color: #fff;
    font-size: 13px;
  }

  /* `.icon-btn` (app.css) supplies the reset; the viewer keeps its own 44px
     circle on a scrim, because these three float over artwork rather than
     sitting in a panel. `#fff` is deliberate: the viewer is always a dark
     overlay, so its controls do not track the theme. */
  .icon {
    position: absolute;
    width: 44px;
    height: 44px;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    transition: background var(--m-fast) ease;
  }

  .icon:hover,
  .icon:focus-visible {
    background: rgba(255, 255, 255, 0.24);
  }

  .close {
    top: 16px;
    right: 16px;
  }

  /* `top: 50%` alone put the button's TOP edge on the centre line, so both
     nav buttons rendered 22px low. The translate is what actually centres
     them. */
  .prev {
    left: 16px;
    top: 50%;
    transform: translateY(-50%);
  }

  .next {
    right: 16px;
    top: 50%;
    transform: translateY(-50%);
  }
</style>
