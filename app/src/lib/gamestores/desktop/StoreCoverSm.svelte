<script lang="ts">
  // The small cover at the top of a store dialog (Install, the cloud
  // conflict, the offline installer). A store's own address is fetched
  // through `storeArtSrc` (the CSP refuses web images); nothing shows
  // until it answers, or when it fails.
  import { storeArtSrc } from '../storeArt';

  let { cover }: { cover: string } = $props();

  let src = $state<string | null>(null);

  $effect(() => {
    const url = cover;
    let live = true;
    src = null;
    void storeArtSrc(url).then((s) => {
      if (live) src = s;
    });
    return () => {
      live = false;
    };
  });
</script>

{#if src}<img class="st-cover-sm" {src} alt="" />{/if}
