<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';

  // One titled, horizontally scrolling row of the Media tab. The arrows
  // page by most of a row and show only when there is somewhere to go.
  let {
    title,
    count,
    testId,
    children,
  }: {
    title: string;
    count: number;
    testId: string;
    children: Snippet;
  } = $props();

  let track = $state<HTMLDivElement | null>(null);
  let canPrev = $state(false);
  let canNext = $state(false);

  function update() {
    if (!track) return;
    canPrev = track.scrollLeft > 1;
    canNext = track.scrollLeft + track.clientWidth < track.scrollWidth - 1;
  }

  function page(direction: 1 | -1) {
    track?.scrollBy({ left: direction * track.clientWidth * 0.8, behavior: 'smooth' });
  }

  // Tiles change width as their images load, and the popup can resize.
  $effect(() => {
    if (!track) return;
    const observer = new ResizeObserver(update);
    observer.observe(track);
    for (const child of track.children) observer.observe(child);
    const mutations = new MutationObserver(() => {
      for (const child of track!.children) observer.observe(child);
      update();
    });
    mutations.observe(track, { childList: true });
    update();
    return () => {
      observer.disconnect();
      mutations.disconnect();
    };
  });
</script>

<section class="carousel" data-testid={testId}>
  <header>
    <h3>{title} <span class="count">{count}</span></h3>
    {#if canPrev || canNext}
      <div class="arrows">
        <button aria-label={`Previous ${title.toLowerCase()}`} disabled={!canPrev} onclick={() => page(-1)}>
          <Icon name="chevronLeft" size={16} />
        </button>
        <button aria-label={`Next ${title.toLowerCase()}`} disabled={!canNext} onclick={() => page(1)}>
          <Icon name="chevronRight" size={16} />
        </button>
      </div>
    {/if}
  </header>
  <div class="track" bind:this={track} onscroll={update}>
    {@render children()}
  </div>
</section>

<style>
  .carousel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .count {
    margin-left: 4px;
    font-weight: 400;
    color: var(--text-muted);
  }

  .arrows {
    display: flex;
    gap: 4px;
  }

  .arrows button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    transition: color var(--m-fast) ease;
  }

  .arrows button:hover:not(:disabled) {
    color: var(--primary);
  }

  .arrows button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .track {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scroll-padding: 0;
    padding-bottom: 6px;
  }

  .track > :global(*) {
    flex: none;
    scroll-snap-align: start;
  }
</style>
