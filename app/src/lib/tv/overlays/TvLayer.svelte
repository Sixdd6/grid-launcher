<script lang="ts">
  // One overlay over the stage (items 360–361; tvmode.js `openLayer`): a
  // scrim with a 6px blur from the top of the window down to the hint bar,
  // which stays in view with the overlay's hints. B closes it unless it is
  // blocking; TV mode ignores the mouse, so the scrim closes nothing. Focus
  // stays inside the top layer (nav.svelte.ts `scope`).
  import type { Layer } from '../nav.svelte';

  let { layer }: { layer: Layer } = $props();
</script>

<div class={`layer ${layer.cls}`} data-layer={layer.id} data-testid="tv-layer" role="presentation">
  <layer.component {...layer.props} />
</div>
