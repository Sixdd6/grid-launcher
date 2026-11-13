<script lang="ts">
  // A store game's screenshot full screen (Details › Media): the media
  // viewer's look, LB / RB (or Left / Right) for the previous and next one,
  // B closes. The art is the store's own, shown as TvStoreArt shows it (never
  // through the RomM image loader).
  import Glyph from '../glyphs/Glyph.svelte';
  import { tvf } from '../focus/actions';
  import TvStoreArt from './TvStoreArt.svelte';

  let { model }: { model: { urls: string[]; index: number; title: string } } = $props();

  let url = $derived(model.urls[model.index] ?? '');
  let caption = $derived(`${model.title} · ${model.index + 1} of ${model.urls.length}`);
</script>

<div class="viewer" data-testid="tv-store-viewer" data-show="shot">
  <div class="col">
    <div class="shot" style="aspect-ratio:16/9"><TvStoreArt {url} alt={caption} placeholder="Screenshot" /></div>
    <div class="cap"><Glyph button="LB" /><span>{caption}</span><Glyph button="RB" /></div>
  </div>
  <button class="sr" data-k="viewer" data-al="Select" use:tvf={{}}>media</button>
</div>
