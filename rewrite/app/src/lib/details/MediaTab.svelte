<script lang="ts">
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import { trailerPoster, type MediaItem } from './media';

  let {
    items,
    onOpen,
    failed,
    onScreenshotError,
    coverUrl,
  }: {
    items: MediaItem[];
    onOpen: (index: number) => void;
    /** URLs whose image failed to load, keyed by URL (owned by Details.svelte). */
    failed: Record<string, true>;
    onScreenshotError: (url: string) => void;
    /** The game's large cover — the poster for a hosted video, and the
     *  fallback poster when YouTube's thumbnail cannot be reached. */
    coverUrl: string | null;
  } = $props();

  // Which YouTube thumbnails have failed, keyed by video id: an offline
  // launcher must fall back to the cover once, not retry on every re-render.
  let thumbnailFailed = $state<Record<string, true>>({});
  function markThumbnailFailed(videoId: string) {
    thumbnailFailed = { ...thumbnailFailed, [videoId]: true };
  }
</script>

{#if items.length}
  <div class="gallery">
    {#each items as item, i (item.caption)}
      {#if !(item.kind === 'screenshot' && failed[item.url])}
        <button
          class="tile"
          data-testid={`details-media-${i}`}
          title={item.caption}
          onclick={() => onOpen(i)}
        >
          {#if item.kind === 'screenshot'}
            <Image
              url={item.url}
              alt={item.caption}
              placeholder="Screenshot"
              onerror={() => onScreenshotError(item.url)}
            />
          {:else if item.kind === 'youtube'}
            {@const poster = trailerPoster(item.videoId, coverUrl, thumbnailFailed[item.videoId] === true)}
            <div class="video-tile">
              {#if poster.kind === 'youtube'}
                <!-- Plain <img>, deliberately NOT `Image.svelte`: that
                     component fetches through `ensure_image` -> RommClient,
                     which would attach the RomM token to a foreign host.
                     `no-referrer` keeps this app's URL out of the request. -->
                <img
                  data-testid={`details-media-thumb-${i}`}
                  class="poster"
                  src={poster.url}
                  alt=""
                  aria-hidden="true"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                  draggable="false"
                  onerror={() => markThumbnailFailed(item.videoId)}
                />
              {:else}
                <Image
                  url={poster.url}
                  alt=""
                  placeholder="Trailer"
                  data-testid={`details-media-poster-${i}`}
                />
              {/if}
              <span class="play-badge" data-testid={`details-media-play-${i}`}>
                <Icon name="play" size={20} />
              </span>
              <span class="video-label">Trailer</span>
            </div>
          {:else}
            <div class="video-tile">
              <Image
                url={coverUrl}
                alt=""
                placeholder="Video"
                data-testid={`details-media-poster-${i}`}
              />
              <span class="play-badge" data-testid={`details-media-play-${i}`}>
                <Icon name="play" size={20} />
              </span>
              <span class="video-label">Video</span>
            </div>
          {/if}
        </button>
      {/if}
    {/each}
  </div>
{:else}
  <p class="empty" data-testid="details-no-media">No media available</p>
{/if}

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
  }

  .tile {
    aspect-ratio: 16 / 9;
    border-radius: var(--r-row);
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 0;
    cursor: pointer;
    display: block;
    width: 100%;
  }

  .tile :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .video-tile {
    position: relative;
    height: 100%;
    width: 100%;
    display: block;
    color: var(--text);
    font-size: 14px;
  }

  /* Both posters fill the tile the same way the screenshot tiles do; the
     `.tile :global(img)` rule above already sizes the <Image> branch. */
  .video-tile .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .play-badge {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-pill);
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
  }

  .video-label {
    position: absolute;
    left: 8px;
    bottom: 6px;
    padding: 2px 8px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 11px;
    font-weight: 600;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
