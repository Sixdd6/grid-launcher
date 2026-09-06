<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';

  let {
    url,
    alt,
    placeholder = alt,
    onerror,
    backdrop = false,
    ...rest
  }: {
    url: string | null;
    alt: string;
    placeholder?: string;
    onerror?: () => void;
    backdrop?: boolean;
    [key: string]: unknown;
  } = $props();
  let src = $state<string | null>(null);
  /**
   * Which of the three things this component can be showing:
   * - `loading` — a URL was given and `ensureImage` has not answered yet.
   *   Renders the shimmer skeleton, NOT the text placeholder: before this,
   *   a tile still being fetched and a tile whose image is gone looked
   *   identical, so a slow cache miss read as a permanent failure.
   * - `error` — there is no URL, or the fetch/decode failed. The text
   *   placeholder is the honest answer here, and it is the state `onerror`
   *   is reported in.
   * - `ready` — `src` is set and the <img> is in the DOM.
   */
  let status = $state<'loading' | 'error' | 'ready'>('loading');

  $effect(() => {
    let cancelled = false;
    src = null;
    // A null/blank url has nothing in flight, so it is `error` (the caller's
    // placeholder text), never a skeleton that would shimmer forever.
    status = url ? 'loading' : 'error';
    if (url) {
      api
        .ensureImage(url)
        .then((path) => {
          if (cancelled) return;
          src = convertFileSrc(path);
          status = 'ready';
        })
        .catch(() => {
          // offline/missing image: the caller decides whether to keep showing
          // the placeholder (covers) or drop the tile entirely (screenshots)
          if (cancelled) return;
          status = 'error';
          onerror?.();
        });
    }
    return () => {
      cancelled = true;
    };
  });

  function handleImgError() {
    src = null;
    status = 'error';
    onerror?.();
  }
</script>

{#if status === 'ready' && src}
  <!-- A decode failure drops back to the placeholder before telling the
       caller: without clearing `src` first, a caller that passes no
       `onerror` (the Library and Server cards) is left with the browser's
       broken-image glyph in the card. -->
  <!-- Eager, not lazy: with `loading="lazy"` every cover was fetched and
       decoded as it scrolled in, and the FIRST scroll through a freshly
       opened 302-card platform averaged 84 ms per frame with 27 frames over
       50 ms; eager + `decoding="async"` makes it 28.6 ms with none, the same
       as a warm second pass. Fetching them all at open is cheap — a cover is
       a local cache file served over the asset protocol, not a network
       request — and decoding asynchronously keeps the open off the main
       thread. -->
  {#if backdrop}
    <img
      class="backdrop"
      src={src}
      alt=""
      aria-hidden="true"
      loading="eager"
      decoding="async"
      draggable="false"
    />
  {/if}
  <img
    {src}
    {alt}
    loading="eager"
    decoding="async"
    draggable="false"
    onerror={handleImgError}
    {...rest}
  />
{:else if status === 'loading'}
  <div class="skeleton" aria-hidden="true" {...rest}></div>
{:else}
  <div class="placeholder" {...rest}>{placeholder}</div>
{/if}

<style>
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 0.8rem;
    text-align: center;
    padding: 8px;
  }

  /* The loading state, and the whole point of the tri-state: a shimmer
     says "still coming", a flat placeholder says "there is nothing here".
     Tokens only — the gradient is two existing surface tokens, so it
     tracks the theme. */
  .skeleton {
    height: 100%;
    width: 100%;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      var(--surface) 25%,
      var(--surface-2) 37%,
      var(--surface) 63%
    );
    background-size: 400% 100%;
    animation: image-shimmer calc(var(--m-slow) * 4) linear infinite;
  }

  @keyframes image-shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .skeleton {
      animation: none;
    }
  }
</style>
