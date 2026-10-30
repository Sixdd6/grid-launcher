<script lang="ts">
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import Carousel from './Carousel.svelte';
  import { mediaSections, trailerPoster, type MediaItem } from './media';

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
  // Screenshots and videos in rows of their own; each tile keeps its
  // full-list index for the viewer and its test id.
  let sections = $derived(mediaSections(items, failed));
  function markThumbnailFailed(videoId: string) {
    thumbnailFailed = { ...thumbnailFailed, [videoId]: true };
  }
</script>

{#if items.length}
  <div class="rows">
    {#if sections.screenshots.length}
      <Carousel title="Screenshots" count={sections.screenshots.length} testId="details-media-screenshots">
        {#each sections.screenshots as { item, index } (item.caption)}
          {#if item.kind === 'screenshot'}
            <button
              class="tile shot"
              data-testid={`details-media-${index}`}
              title={item.caption}
              onclick={() => onOpen(index)}
            >
              <Image
                url={item.url}
                alt={item.caption}
                placeholder="Screenshot"
                onerror={() => onScreenshotError(item.url)}
              />
            </button>
          {/if}
        {/each}
      </Carousel>
    {/if}

    {#if sections.videos.length}
      <Carousel title="Videos" count={sections.videos.length} testId="details-media-videos">
        {#each sections.videos as { item, index } (item.caption)}
          <button
            class="tile video"
            data-testid={`details-media-${index}`}
            title={item.caption}
            onclick={() => onOpen(index)}
          >
            {#if item.kind === 'youtube'}
              {@const poster = trailerPoster(item.videoId, coverUrl, thumbnailFailed[item.videoId] === true)}
              <div class="video-tile">
                {#if poster.kind === 'youtube'}
                  <!-- Plain <img>, deliberately NOT `Image.svelte`: that
                       component fetches through `ensure_image` -> RommClient,
                       which would attach the RomM token to a foreign host.
                       `no-referrer` keeps this app's URL out of the request. -->
                  <img
                    data-testid={`details-media-thumb-${index}`}
                    class="poster"
                    src={poster.url}
                    alt=""
                    aria-hidden="true"
                    loading="lazy"
                    referrerpolicy="no-referrer"
                    draggable="false"
                    onerror={() => markThumbnailFailed(item.videoId)}
                    onload={(e) => {
                      // YouTube answers a missing thumbnail with HTTP 200 and a
                      // grey 120×90 placeholder, so `onerror` never fires for it.
                      if ((e.currentTarget as HTMLImageElement).naturalWidth <= 120) markThumbnailFailed(item.videoId);
                    }}
                  />
                {:else}
                  <Image
                    url={poster.url}
                    alt=""
                    placeholder="Trailer"
                    data-testid={`details-media-poster-${index}`}
                  />
                {/if}
                <span class="play-badge" data-testid={`details-media-play-${index}`}>
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
                  data-testid={`details-media-poster-${index}`}
                />
                <span class="play-badge" data-testid={`details-media-play-${index}`}>
                  <Icon name="play" size={20} />
                </span>
                <span class="video-label">Video</span>
              </div>
            {/if}
          </button>
        {/each}
      </Carousel>
    {/if}
  </div>
{:else}
  <p class="empty" data-testid="details-no-media">No media available</p>
{/if}

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* One tile height for both rows. A screenshot keeps its own shape (a
     4:3 console frame is not cropped to 16:9); a video tile is 16:9. */
  .tile {
    height: 150px;
    border-radius: var(--r-row);
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 0;
    cursor: pointer;
    display: block;
  }

  .tile.video {
    aspect-ratio: 16 / 9;
  }

  .tile.shot :global(img) {
    display: block;
    height: 100%;
    width: auto;
  }

  /* Until a screenshot loads, its skeleton or placeholder holds a 16:9
     space, so the row does not start at zero width. */
  .tile.shot :global(.skeleton),
  .tile.shot :global(.placeholder) {
    width: calc(150px * 16 / 9);
  }

  .tile.video :global(img) {
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
