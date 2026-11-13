<script lang="ts">
  // A store game's art. The store's covers and screenshots are either a
  // file GRID cached (a path) or an address of the store's own. An address
  // goes through `storeArtSrc` (the CSP refuses web images): GRID caches it
  // with no credential, never through the RomM image loader with the
  // server's token (TvMediaTab's YouTube posters follow the same rule).
  import { storeArtSrc } from '../../gamestores/storeArt';

  let { url, alt, placeholder = alt, onmeasure = undefined }: { url: string; alt: string; placeholder?: string; onmeasure?: (w: number, h: number) => void } = $props();

  let failed = $state(false);
  let src = $state('');

  $effect(() => {
    const u = url;
    let live = true;
    failed = false;
    src = '';
    void storeArtSrc(u).then((s) => {
      if (!live) return;
      if (s === null) failed = u.trim() !== '';
      else src = s;
    });
    return () => {
      live = false;
    };
  });
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
