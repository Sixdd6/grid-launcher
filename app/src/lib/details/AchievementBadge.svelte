<script lang="ts">
  // One achievement badge, loaded only once its row comes near the visible
  // part of the tab: a game can list hundreds of achievements, and each
  // badge is a cache fetch (`ensureImage`) on a first view.
  //
  // Three looks: the shimmer while the badge waits to load (the same gradient
  // as `Image`'s skeleton), the image itself, and a neutral trophy when the
  // server gave no badge or the image could not be loaded.
  import Image from '../Image.svelte';
  import { scrollParent } from '../visibleWarm';

  let {
    url,
    alt,
    dimmed = false,
    ring = null,
  }: {
    url: string;
    alt: string;
    /** A locked achievement: the picture is desaturated and faded. */
    dimmed?: boolean;
    /** `hardcore` draws a gold ring round the badge. */
    ring?: 'hardcore' | null;
  } = $props();

  let near = $state(false);
  let slot = $state<HTMLElement | null>(null);
  // The url that failed to load, so a new url starts fresh without an effect.
  let failedUrl = $state<string | null>(null);
  let failed = $derived(failedUrl !== null && failedUrl === url);

  $effect(() => {
    const el = slot;
    if (el === null || near) return;
    // No IntersectionObserver (node test runner): load at once.
    if (typeof IntersectionObserver === 'undefined') {
      near = true;
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          near = true;
          observer.disconnect();
        }
      },
      { root: scrollParent(el), rootMargin: '200px 0px' },
    );
    observer.observe(el);
    return () => observer.disconnect();
  });
</script>

<span class="badge" class:dimmed class:hardcore={ring === 'hardcore'} bind:this={slot}>
  {#if !url || failed}
    <svg
      class="trophy"
      viewBox="0 0 24 24"
      width="26"
      height="26"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M8 4h8v5a4 4 0 0 1-8 0V4z" />
      <path d="M8 6H5v1.5A3 3 0 0 0 8 10.5M16 6h3v1.5a3 3 0 0 1-3 3" />
      <path d="M12 13v3M9.5 20h5M10.5 16h3v4h-3z" />
    </svg>
  {:else if near}
    <Image {url} {alt} placeholder="" onerror={() => (failedUrl = url)} />
  {:else}
    <span class="badge-empty" aria-hidden="true"></span>
  {/if}
</span>

<style>
  .badge {
    display: block;
    width: 48px;
    height: 48px;
    flex: none;
    border-radius: var(--r-chip);
    overflow: hidden;
    background: var(--surface-2);
  }

  /* A ring outside the badge's own clipped box. */
  .badge.hardcore {
    box-shadow: 0 0 0 2px var(--gold);
  }

  .badge :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  /* A locked badge reads as "not yet": desaturated and faded. Only the
     picture is dimmed; the row's text keeps its own full-contrast colours. */
  .badge.dimmed :global(img) {
    filter: grayscale(1);
    opacity: 0.55;
  }

  .trophy {
    display: block;
    margin: 11px; /* (48 - 26) / 2 */
    color: var(--text-muted);
    opacity: 0.8;
  }

  /* The same gradient and pace as Image's loading skeleton, so a badge
     waiting for its row to scroll near looks like one being fetched. */
  .badge-empty {
    display: block;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      90deg,
      var(--surface) 25%,
      var(--surface-2) 37%,
      var(--surface) 63%
    );
    background-size: 400% 100%;
    animation: badge-shimmer calc(var(--m-slow) * 4) linear infinite;
  }

  @keyframes badge-shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .badge-empty {
      animation: none;
    }
  }
</style>
