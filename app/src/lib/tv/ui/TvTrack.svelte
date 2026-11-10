<script lang="ts">
  // A row that scrolls sideways (items 26, 82, 110): covers in a line,
  // 1.4rem above and below for a focused cover's growth and ring, and the
  // focused cover kept 1.2rem inside its edges (ruling 3). `up` / `down` are
  // explicit links (Details' Related row goes Up to the summary, item 91).
  import type { Snippet } from 'svelte';

  let {
    scroll,
    up = undefined,
    down = undefined,
    enter = undefined,
    children,
  }: { scroll: string; up?: string; down?: string; enter?: 'first' | 'sel'; children: Snippet } = $props();
</script>

<div class="track" data-nav="h" data-track data-scroll={scroll} data-up={up} data-down={down} data-enter={enter}>
  {@render children()}
</div>
