<script lang="ts">
  // RomM's mark: a monogram tile "R" in RomM's colour, never a logo
  // (TVRommStorefront README § Library › RomM group; prototype `rmark`).
  // `lg` is the 3rem tile of the Settings row, the RomM page and the welcome.
  let { lg = false }: { lg?: boolean } = $props();
</script>

<span class="smono" class:lg style="--h:hsl(258 50% 46%)" aria-hidden="true" data-mono="R"></span>
