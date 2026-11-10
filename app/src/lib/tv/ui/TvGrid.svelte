<script lang="ts">
  // A grid of covers (items 26–27, 88–90): 10rem columns filling the width,
  // inside a scroll area that reaches 1.2rem past the header on both sides.
  // Moving off its edge reaches only what sits straight past that edge; it
  // is entered where it was left, else at its first cover.
  // TODO(P7/P8): a windowed grid for large platforms (lib/cards/window.ts)
  // moves by index inside the grid (focus/grid.ts `gridMove`).
  import type { Snippet } from 'svelte';

  let { id, scroll, children = undefined }: { id: string; scroll: string; children?: Snippet } = $props();
</script>

<div class="scroll" data-scroll={scroll}>
  <div class="grid" {id} data-nav="grid" data-enter="first" data-rail-target>
    {@render children?.()}
  </div>
</div>
