<script lang="ts">
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import type { MediaItem } from './media';

  let {
    items,
    onOpen,
  }: {
    items: MediaItem[];
    onOpen: (index: number) => void;
  } = $props();
</script>

{#if items.length}
  <div class="gallery">
    {#each items as item, i (item.caption)}
      <button
        class="tile"
        data-testid={`details-media-${i}`}
        title={item.caption}
        onclick={() => onOpen(i)}
      >
        {#if item.kind === 'screenshot'}
          <Image url={item.url} alt={item.caption} placeholder="Screenshot" />
        {:else}
          <div class="video-tile">
            <Icon name="play" size={20} />
            <span>{item.kind === 'youtube' ? 'Trailer' : 'Video'}</span>
          </div>
        {/if}
      </button>
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
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
