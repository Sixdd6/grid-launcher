<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from '../api';
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import { nextIndex, prevIndex, youtubeEmbedUrl, type MediaItem } from './media';

  let {
    items,
    index,
    onIndex,
    onClose,
    failed,
    onScreenshotError,
  }: {
    items: MediaItem[];
    index: number;
    onIndex: (index: number) => void;
    onClose: () => void;
    failed: Record<string, true>;
    onScreenshotError: (url: string) => void;
  } = $props();

  let viewerEl = $state<HTMLElement | null>(null);
  let current = $derived(items[index] ?? null);

  // A server-hosted video is fetched through the session client and played
  // from the local cache (`ensure_video`, Task 4). The server URL never
  // reaches the DOM, so no request from the page needs a token.
  let videoSrc = $state<string | null>(null);
  let videoError = $state(false);

  $effect(() => {
    const item = current;
    videoSrc = null;
    videoError = false;
    if (item === null || item.kind !== 'video') return;
    let cancelled = false;
    api
      .ensureVideo(item.url)
      .then((path) => {
        if (!cancelled) videoSrc = convertFileSrc(path);
      })
      .catch(() => {
        if (!cancelled) videoError = true;
      });
    return () => {
      cancelled = true;
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
        <iframe
          data-testid="media-viewer-youtube"
          class="frame"
          src={youtubeEmbedUrl(current.videoId)}
          title={current.caption}
          allow="accelerometer; clipboard-write; encrypted-media; picture-in-picture"
          allowfullscreen
        ></iframe>
      {:else if videoSrc}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video data-testid="media-viewer-video" class="frame" src={videoSrc} controls></video>
      {:else}
        <p class="pending" data-testid="media-viewer-video-pending">
          {videoError ? 'This video could not be loaded' : 'Loading video…'}
        </p>
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
