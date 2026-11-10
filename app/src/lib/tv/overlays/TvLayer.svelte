<script lang="ts">
  // One overlay over the stage (items 360–361; tvmode.js `openLayer`): a
  // scrim with a 6px blur from the top of the window down to the hint bar,
  // which stays in view with the overlay's hints. A click on the scrim
  // itself closes it, unless it is blocking (item 76). Focus stays inside
  // the top layer (nav.svelte.ts `scope`).
  import { closeLayer, topLayer, type Layer } from '../nav.svelte';

  let { layer }: { layer: Layer } = $props();

  function outside(e: MouseEvent) {
    if (e.target !== e.currentTarget || layer.blocking || topLayer()?.id !== layer.id) return;
    void closeLayer();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class={`layer ${layer.cls}`} data-layer={layer.id} data-testid="tv-layer" role="presentation" onclick={outside}>
  <layer.component {...layer.props} />
</div>
