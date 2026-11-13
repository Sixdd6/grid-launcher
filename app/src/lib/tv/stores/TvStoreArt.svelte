<script lang="ts">
  // A store game's art. The store's covers and screenshots are either a
  // file GRID cached (a path) or an address of the store's own; neither goes
  // through the RomM image loader, which would send the server's token
  // with it (TvMediaTab's YouTube posters follow the same rule).
  import { fileSrc } from '../../fileSrc';

  let { url, alt, placeholder = alt, onmeasure = undefined }: { url: string; alt: string; placeholder?: string; onmeasure?: (w: number, h: number) => void } = $props();

  let failed = $state(false);
  let src = $derived(url === '' ? '' : /^(data:|https?:|blob:)/.test(url) ? url : fileSrc(url));
</script>

{#if src && !failed}
  <img
    class="sart"
    {src}
    {alt}
    referrerpolicy="no-referrer"
    draggable="false"
    onerror={() => (failed = true)}
    onload={(e) => {
      const img = e.currentTarget as HTMLImageElement;
      onmeasure?.(img.naturalWidth, img.naturalHeight);
    }}
  />
{:else}
  <span class="sart-ph">{placeholder}</span>
{/if}
