<script lang="ts">
  // A store's mark: a monogram tile in the store's colour, never its logo
  // (GameStores designer decision 3; prototype `smark`). `lg` is the 3rem
  // tile of the panels, Settings rows and the store's page.
  import type { StoreSource } from '../../api';
  import { storeMark } from '../../gamestores/marks';

  let { source, lg = false }: { source: StoreSource; lg?: boolean } = $props();

  let m = $derived(storeMark(source));
</script>

<span class="smono" class:lg style={`--h:${m.hue}`} aria-hidden="true">{m.mono}</span>
