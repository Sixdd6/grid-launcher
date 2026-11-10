<script lang="ts">
  // A rail (items 14, 82, 85–86, 105–106): an 18rem panel of rows that keeps
  // focus inside it vertically and is always entered on its selected row.
  // The rows are TvRailRow; headings (`<h3>`), dividers (`<hr>`) and notes
  // (`.ri-empty`) go between them.
  import type { Snippet } from 'svelte';

  let { label, scroll = 'rail', children }: { label: string; scroll?: string; children: Snippet } = $props();
</script>

<nav class="rail" data-nav="v" data-enter="sel" data-rail data-scroll={scroll} aria-label={label}>
  {@render children()}
</nav>
