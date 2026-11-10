<script lang="ts">
  // Details › Media (items 301–302; tvmode.js `detBody`): a Screenshots row
  // and a Videos row with their counts, tiles 8rem tall at each image's own
  // shape, a play icon on a video tile. A opens the fullscreen viewer on the
  // tile. A screenshot whose image fails leaves the row (the desktop's rule).
  import Image from '../../../Image.svelte';
  import type { DetailsStore } from '../../../details/detailsStore.svelte';
  import { mediaSections, trailerPoster, viewableItems, type MediaItem } from '../../../details/media';
  import { tvf } from '../../focus/actions';
  import TvIcon from '../../ui/TvIcon.svelte';
  import { mediaAction } from '../details';
  import { openViewer } from '../mediaViewer.svelte';

  let { d }: { d: DetailsStore } = $props();

  let sections = $derived(mediaSections(d.mediaItems, d.failedMedia));
  /** Each screenshot's width / height once its image is in (16:9 until then). */
  let ratios = $state<Record<string, number>>({});
  let thumbFailed = $state<Record<string, true>>({});
  let cover = $derived(d.coverLarge ?? d.coverSmall);

  function open(item: MediaItem) {
    const items = viewableItems(d.mediaItems, d.failedMedia);
    openViewer({ items, index: Math.max(0, items.indexOf(item)), title: d.subject.name, cover, onFail: (url) => d.markMediaFailed(url) });
  }
</script>

{#if sections.screenshots.length || sections.videos.length}
  {#if sections.screenshots.length}
    <div class="sub-h">Screenshots<span>{sections.screenshots.length}</span></div>
    <div class="mtrack" data-nav="h" data-enter="first" data-track data-scroll="det-shots" data-testid="tv-det-shots">
      {#each sections.screenshots as { item, index } (index)}
        {#if item.kind === 'screenshot'}
          <button
            class="mtile"
            data-k={`m${index}`}
            data-al={mediaAction(item)}
            data-testid={`tv-det-media-${index}`}
            style={`width:${8 * (ratios[item.url] ?? 16 / 9)}rem`}
            use:tvf={{ a: () => open(item) }}
          >
            <Image
              url={item.url}
              alt={item.caption}
              placeholder="Screenshot"
              onmeasure={(w, h) => (ratios = { ...ratios, [item.url]: w / h })}
              onerror={() => d.markMediaFailed(item.url)}
            />
          </button>
        {/if}
      {/each}
    </div>
  {/if}
  {#if sections.videos.length}
    <div class="sub-h">Videos<span>{sections.videos.length}</span></div>
    <div class="mtrack" data-nav="h" data-enter="first" data-track data-scroll="det-videos" data-testid="tv-det-videos">
      {#each sections.videos as { item, index } (index)}
        <button
          class="mtile video"
          data-k={`m${index}`}
          data-al={mediaAction(item)}
          data-testid={`tv-det-media-${index}`}
          style="width:14.2rem"
          use:tvf={{ a: () => open(item) }}
        >
          {#if item.kind === 'youtube'}
            {@const poster = trailerPoster(item.videoId, cover, thumbFailed[item.videoId] === true)}
            {#if poster.kind === 'youtube'}
              <!-- Plain <img>, not Image.svelte, which would send the RomM
                   token to a foreign host (MediaTab.svelte). -->
              <img
                src={poster.url}
                alt=""
                referrerpolicy="no-referrer"
                draggable="false"
                onerror={() => (thumbFailed = { ...thumbFailed, [item.videoId]: true })}
                onload={(e) => {
                  if ((e.currentTarget as HTMLImageElement).naturalWidth <= 120) thumbFailed = { ...thumbFailed, [item.videoId]: true };
                }}
              />
            {:else}
              <Image url={poster.url} alt="" placeholder="Trailer" />
            {/if}
          {:else}
            <Image url={cover} alt="" placeholder="Video" />
          {/if}
          <span class="play"><TvIcon name="play" px={36} /></span>
        </button>
      {/each}
    </div>
  {/if}
{:else}
  <p class="note" data-testid="tv-det-no-media">No media available</p>
{/if}
