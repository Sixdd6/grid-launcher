<script lang="ts" generics="T">
  import { untrack, type Snippet } from 'svelte';
  import { cardMinPx, labelRowPx, targetRowPx, type CardSize } from './cards/size';
  import type { CardLabels } from './cards/labels';
  import {
    BUFFER_PX,
    columnsLayout,
    INITIAL_WINDOW_CARDS,
    neighbor as neighborOf,
    rowsLayout,
    visibleRange,
    type CardDisplay,
    type Layout,
    type NavDirection,
    type Rect,
  } from './cards/layout';
  import { shapeOf } from './stores/coverShapes.svelte';
  import { scrollParent } from './visibleWarm';

  let {
    items,
    key,
    cover,
    card,
    size,
    labels,
    display,
    gridId,
  }: {
    /** The view's whole list. Only the cards near the visible part render. */
    items: readonly T[];
    key: (item: T) => string | number;
    /** The item's cover URL: its shape decides the card's size. */
    cover: (item: T) => string | null;
    /** One card, given its item, its index in `items`, and its cover's
     *  rectangle (`cards/layout.ts`). The card positions itself by it. */
    card: Snippet<[T, number, Rect]>;
    size: CardSize;
    /** The "Game labels" setting: it decides the label row's height. */
    labels: CardLabels;
    /** The "Game card display" setting: Equal height or Equal width. */
    display: CardDisplay;
    gridId: string;
  } = $props();

  /** The grid's own padding above the first card and below the last. */
  const PAD_TOP_PX = 18;
  const PAD_BOTTOM_PX = 24;
  /** Side room so a lifted card's shadow is not clipped by the scroller. */
  const PAD_SIDE_PX = 4;
  /** The width laid out before the grid has been measured (first paint,
   *  server-side rendering, a view mounted hidden). */
  const FALLBACK_WIDTH = 1280;

  let el = $state<HTMLElement | null>(null);
  /** The content width, once measured. */
  let width = $state<number | null>(null);
  /** The visible band, in px from the first card's top; `null` until
   *  measured. */
  let band = $state.raw<{ top: number; bottom: number } | null>(null);

  let labelRow = $derived(labelRowPx(labels));
  let shapes = $derived(items.map((item) => shapeOf(cover(item))));
  let layout: Layout = $derived.by(() => {
    const w = width ?? FALLBACK_WIDTH;
    return display === 'columns'
      ? columnsLayout({ shapes, width: w, labelRow, minColumn: cardMinPx(size) })
      : rowsLayout({ shapes, width: w, labelRow, target: targetRowPx(size) });
  });
  let shown = $derived(
    band === null
      ? { start: 0, end: Math.min(items.length, INITIAL_WINDOW_CARDS) }
      : visibleRange(layout, labelRow, band.top - BUFFER_PX, band.bottom + BUFFER_PX)
  );
  let visible = $derived(items.slice(shown.start, shown.end));

  // Plain, not state: read on every scroll event.
  let scroller: Element | null = null;

  /** Reads the width and the visible band. Ignores a grid with no layout: a
   *  `hidden` view measures zero, and the last measurement still holds. */
  function measure(): void {
    const grid = el;
    if (!grid) return;
    const rect = grid.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return;
    const w = Math.floor(grid.clientWidth - 2 * PAD_SIDE_PX);
    if (w > 0 && w !== width) width = w;
    let viewTop = 0;
    let viewportHeight = window.innerHeight;
    if (scroller) {
      viewTop = scroller.getBoundingClientRect().top + scroller.clientTop;
      viewportHeight = scroller.clientHeight;
    }
    if (!(viewportHeight > 0)) return;
    const top = viewTop - (rect.top + PAD_TOP_PX);
    const next = { top, bottom: top + viewportHeight };
    // A new band only when it moved far enough to matter: a scroll inside
    // the buffer costs two rect reads and no render.
    if (band !== null && Math.abs(band.top - next.top) < BUFFER_PX / 4 && band.bottom - band.top === viewportHeight) return;
    band = next;
  }

  $effect(() => {
    const grid = el;
    if (!grid) return;
    scroller = scrollParent(grid);
    const target: Element | Window = scroller ?? window;
    const onScroll = () => measure();
    // Passive: this handler never calls `preventDefault`.
    target.addEventListener('scroll', onScroll, { passive: true });
    // The grid's width decides the layout; the container's height decides
    // how much is visible. Both change when a hidden view is shown again.
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => measure());
    resize?.observe(grid);
    if (scroller) resize?.observe(scroller);
    untrack(() => measure());
    return () => {
      target.removeEventListener('scroll', onScroll);
      resize?.disconnect();
      scroller = null;
    };
  });

  /** The card arrow-key movement goes to (`cards/layout.ts`). */
  export function neighbor(index: number, dir: NavDirection): number {
    return neighborOf(layout.rects, index, dir);
  }

  /** The grid element. Its children are the rendered cards only. */
  export function element(): HTMLElement | null {
    return el;
  }

  /** The rendered slice of `items`. Reactive: an effect that reads it runs
   *  again when the window moves. */
  export function range(): { start: number; end: number } {
    return shown;
  }

  /**
   * Brings card `index` into view: a smooth `nearest` scroll when it is
   * rendered (keyboard movement goes one card at a time, and the buffer is
   * rendered), otherwise an instant jump to its rectangle, which renders it.
   */
  export function scrollToIndex(index: number): void {
    const grid = el;
    const rect = layout.rects[index];
    if (!grid || !rect) return;
    if (index >= shown.start && index < shown.end) {
      grid.children[index - shown.start]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      return;
    }
    const gridTop = grid.getBoundingClientRect().top + PAD_TOP_PX;
    const viewTop = scroller ? scroller.getBoundingClientRect().top : 0;
    const current = scroller ? scroller.scrollTop : window.scrollY;
    const top = Math.max(0, current + (gridTop - viewTop) + rect.y - 24);
    if (scroller) scroller.scrollTop = top;
    else window.scrollTo(0, top);
    measure();
  }
</script>

<!-- D-UI-7: grids may run to the full window width, capped at 1920px. They
     deliberately do NOT take `.view-content` (1100px), which is for lists.
     Every card is placed by its rectangle from `cards/layout.ts`, and only
     the cards near the visible part are rendered; the grid's own height is
     the whole layout's, so the scrollbar matches the whole list. Every child
     of the grid is a card. -->
<div
  data-testid={gridId}
  class="grid"
  data-display={display}
  bind:this={el}
  style="--height: {layout.height}px; --pad-top: {PAD_TOP_PX}px; --pad-bottom: {PAD_BOTTOM_PX}px; --pad-side: {PAD_SIDE_PX}px"
>
  {#each visible as item, j (key(item))}
    {@render card(item, shown.start + j, layout.rects[shown.start + j])}
  {/each}
</div>

<style>
  .grid {
    position: relative;
    box-sizing: content-box;
    /* `flex: none`: the grid sits in the view's scrolling flex column, and
       its absolutely placed cards give it no minimum size, so the column
       would otherwise shrink it to the screen and the list would stop
       scrolling there. */
    flex: none;
    height: var(--height);
    padding: var(--pad-top) var(--pad-side) var(--pad-bottom);
    width: calc(100% - 2 * var(--pad-side));
    max-width: 1920px;
    margin: 0 auto;
    /* The window changes which cards exist on its own; the browser's scroll
       anchoring would "correct" the scroll position for that and fight it. */
    overflow-anchor: none;
  }
</style>
