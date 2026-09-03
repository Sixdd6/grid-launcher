<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';

  let {
    url,
    alt,
    placeholder = alt,
    ...rest
  }: { url: string | null; alt: string; placeholder?: string; [key: string]: unknown } = $props();
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
        .catch(() => {}); // offline/missing image: placeholder stays
    }
    return () => {
      cancelled = true;
    };
  });
</script>

{#if src}
  <img {src} {alt} loading="lazy" draggable="false" {...rest} />
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
