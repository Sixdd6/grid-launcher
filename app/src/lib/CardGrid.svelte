<script lang="ts">
  import { columnsOf, gridTemplate, type CardSize } from './cards/size';

  let {
    size,
    gridId,
    children,
  }: {
    size: CardSize;
    gridId: string;
    children: import('svelte').Snippet;
  } = $props();

  let el = $state<HTMLElement | null>(null);

  /** The number of columns the browser resolved, for arrow-key movement. */
  export function columns(): number {
    return columnsOf(el);
  }

  /** The grid element, so a view can scroll its focused child into view. */
  export function element(): HTMLElement | null {
    return el;
  }
</script>

<!-- D-UI-7: grids may run to the full window width, capped at 1920px. They
     deliberately do NOT take `.view-content` (1100px), which is for lists. -->
<div
  data-testid={gridId}
  class="grid"
  bind:this={el}
  style="--template: {gridTemplate(size)}"
>
  {@render children()}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: var(--template);
    gap: 16px;
    padding: 16px 24px 24px;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }
</style>
