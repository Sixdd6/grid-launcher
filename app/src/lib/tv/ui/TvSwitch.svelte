<script lang="ts">
  // A switch (item 29): 3.4 × 1.9rem, `primary` filled when on. Drawn inside
  // a focusable row; the row flips it.
  let { on }: { on: boolean } = $props();
</script>

<span class="sw" class:on role="switch" aria-checked={on}></span>
