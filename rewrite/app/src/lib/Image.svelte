<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';

  let {
    url,
    alt,
    placeholder = alt,
    onerror,
    ...rest
  }: {
    url: string | null;
    alt: string;
    placeholder?: string;
    onerror?: () => void;
    [key: string]: unknown;
  } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    let cancelled = false;
    src = null;
    if (url) {
      api
        .ensureImage(url)
        .then((path) => {
          if (!cancelled) src = convertFileSrc(path);
        })
        .catch(() => {
          // offline/missing image: placeholder stays, caller decides whether
          // to keep showing it (covers) or drop the tile entirely (screenshots)
          if (!cancelled) onerror?.();
        });
    }
    return () => {
      cancelled = true;
    };
  });

  function handleImgError() {
    src = null;
    onerror?.();
  }
</script>

{#if src}
  <!-- A decode failure drops back to the placeholder before telling the
       caller: without clearing `src` first, a caller that passes no
       `onerror` (the Library and Server cards) is left with the browser's
       broken-image glyph in the card. -->
  <img {src} {alt} loading="lazy" draggable="false" onerror={handleImgError} {...rest} />
{:else}
  <div class="placeholder" {...rest}>{placeholder}</div>
{/if}

<style>
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    background: #2a2d34;
    color: #aab;
    font-size: 0.8rem;
    text-align: center;
    padding: 8px;
  }
</style>
