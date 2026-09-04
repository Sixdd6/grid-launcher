<script lang="ts">
  import type { RomDetail } from '../api';
  import Image from '../Image.svelte';
  import { galleryItems } from './media';

  let {
    name,
    screenshotUrls,
    detail,
  }: {
    name: string;
    screenshotUrls: string[];
    detail: RomDetail | null;
  } = $props();

  let items = $derived(
    galleryItems({
      title: name,
      screenshotUrls,
      youtubeVideoId: detail?.youtube_video_id ?? '',
      videoPath: detail?.video_path ?? '',
    })
  );
</script>

{#if items.length}
  <div class="gallery">
    {#each items as item, i (item.caption)}
      <div class="tile" data-testid={`details-media-${i}`} title={item.caption}>
        {#if item.kind === 'screenshot'}
          <Image url={item.url} alt={item.caption} placeholder="Screenshot" />
        {:else}
          <div class="video-tile">▶ {item.kind === 'youtube' ? 'Trailer' : 'Video'}</div>
        {/if}
      </div>
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
  }

  .tile :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .video-tile {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--text);
    font-size: 14px;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
