<script lang="ts">
  import { fileSrc } from './fileSrc';
  import { api } from './api';
  import { imagePaths, startImageLoad, watchNear, type LoadSlot } from './imageLoad';
  import { coverFit, type CoverFit } from './letterbox';

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

  // The skeleton, watched while it waits: its slot is near-priority while
  // the skeleton is near the visible area and far otherwise (`imageLoad.ts`
  // has why). It unmounts once the image is ready, which ends the watch.
  let skeletonEl = $state<HTMLElement | null>(null);

  /** This url's load slot. Held from the `ensure_image` call until the
   *  `<img>` has loaded or failed, so the bound covers the asset request as
   *  well as the IPC call. Raw: the limiter owns it, nothing renders it. */
  let slot = $state.raw<LoadSlot | null>(null);

  $effect(() => {
    const current = slot;
    const el = skeletonEl;
    if (!current || !el) return;
    return watchNear(el, (near) => current.setPriority(near ? 'near' : 'far'));
  });

  /** Bumped to load again after a remembered file failed to decode. */
  let retry = $state(0);
  /** Whether the `<img>` on screen came from `imagePaths`, not a fresh
   *  `ensure_image`. Plain: only the error handler reads it. */
  let remembered = false;

  $effect(() => {
    void retry;
    let cancelled = false;
    src = null;
    slot = null;
    // A null/blank url has nothing in flight, so it is `error` (the caller's
    // placeholder text), never a skeleton that would shimmer forever.
    status = url ? 'loading' : 'error';
    if (!url) return;
    // A card the windowed grid unmounted and mounted again: the file is
    // already on disk, so render it now instead of queueing behind the
    // images that have never loaded.
    const known = imagePaths.get(url);
    remembered = known !== undefined;
    if (known !== undefined) {
      src = fileSrc(known);
      status = 'ready';
      return;
    }
    // Far until the watch above says otherwise: an image that mounts off
    // screen must not go ahead of the ones on it.
    const load = startImageLoad(url, (u) => api.ensureImage(u));
    slot = load.slot;
    load.path
      .then((path) => {
        if (cancelled) return;
        src = fileSrc(path);
        status = 'ready';
      })
      .catch(() => {
        // offline/missing image: the caller decides whether to keep showing
        // the placeholder (covers) or drop the tile entirely (screenshots)
        if (cancelled) return;
        status = 'error';
        onerror?.();
      });
    return () => {
      cancelled = true;
      load.cancel();
    };
  });

  /** How the loaded cover sits in its frame, for a `backdrop` image only
   *  (`letterbox.ts`). `null` until it has loaded: a backdrop behind art that
   *  fills its frame is never seen, and it is the most expensive thing a card
   *  paints. `crop` is set on the `<img>` as `data-fit` so the card can fill
   *  the frame with near-3:4 art instead. */
  let fit = $state<CoverFit | null>(null);

  $effect(() => {
    void url;
    fit = null;
  });

  function handleImgLoad(e: Event) {
    slot?.release();
    if (!backdrop) return;
    const img = e.currentTarget as HTMLImageElement;
    fit = coverFit(img.naturalWidth, img.naturalHeight, img.clientWidth, img.clientHeight);
  }

  function handleImgError() {
    slot?.release();
    if (remembered && url) {
      // The remembered file is gone or broken (the cache was cleared): forget
      // it and load once more the normal way, which fetches it again.
      imagePaths.delete(url);
      remembered = false;
      retry += 1;
      return;
    }
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
       as a warm second pass. Decoding asynchronously keeps the open off the
       main thread. What loads is bounded before this tag ever mounts: at
       most a dozen images at once, the ones near the visible area first
       (`imageLoad.ts`) — fetching all of a 2000-game platform at open
       exhausted the web process's file descriptors. -->
  {#if backdrop && fit === 'letterbox'}
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
    data-fit={fit === 'crop' ? 'crop' : undefined}
    onload={handleImgLoad}
    onerror={handleImgError}
    {...rest}
  />
{:else if status === 'loading'}
  <div class="skeleton" aria-hidden="true" bind:this={skeletonEl} {...rest}></div>
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
