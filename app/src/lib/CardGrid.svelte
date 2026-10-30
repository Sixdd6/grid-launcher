<script lang="ts" generics="T">
  import { untrack, type Snippet } from 'svelte';
  import { CARD_CELL_ASPECT, columnsOf, gridTemplate, labelRowPx, parseTracks, type CardSize } from './cards/size';
  import type { CardLabels } from './cards/labels';
  import {
    BUFFER_ROWS,
    GRID_GAP_PX,
    initialRange,
    rowMetrics,
    scrollTopForIndex,
    windowRange,
    type RowMetrics,
    type WindowRange,
  } from './cards/window';
  import { scrollParent } from './visibleWarm';

  let {
    items,
    key,
    card,
    size,
    labels,
    cellAspect = CARD_CELL_ASPECT,
    gridId,
    bufferRows = BUFFER_ROWS,
  }: {
    /** The view's whole list. Only the rows near the visible part render. */
    items: readonly T[];
    key: (item: T) => string | number;
    /** One card, given its item and its index in `items`. */
    card: Snippet<[T, number]>;
    size: CardSize;
    /** The "Game labels" setting: it decides the label row's height. */
    labels: CardLabels;
    /** The cells' width over height (`cards/shape.ts`). */
    cellAspect?: number;
    gridId: string;
    bufferRows?: number;
  } = $props();

  /** The grid's own top padding, in px, above the first row. */
  const PAD_TOP_PX = 18;
  /** The grid's own bottom padding, in px, below the last row. */
  const PAD_BOTTOM_PX = 24;

  type Geometry = { offset: number; viewportHeight: number; pitch: number; columns: number };

  let el = $state<HTMLElement | null>(null);
  /** The last measurement that changed the window; `null` until the grid
   *  has been laid out (the first paint, SSR, a view mounted hidden). */
  let geometry = $state.raw<Geometry | null>(null);
  /** The row height the browser lays out, so its rows match the math. */
  let rowHeight = $state<number | null>(null);

  let shown: WindowRange = $derived(
    geometry === null
      ? initialRange(items.length)
      : windowRange({ ...geometry, count: items.length, bufferRows })
  );
  let visible = $derived(items.slice(shown.start, shown.end));

  // Plain, not state: read on every scroll event, and only `geometry`
  // decides what renders.
  let scroller: Element | null = null;
  let metrics: RowMetrics | null = null;
  let trackCount = 1;

  /** Reads the resolved columns. Ignores a grid with no layout: a `hidden`
   *  view measures zero, and the last real measurement still holds. */
  function measureTracks(): void {
    const grid = el;
    if (!grid) return;
    const tracks = parseTracks(getComputedStyle(grid).gridTemplateColumns);
    if (tracks === null) return;
    trackCount = tracks.length;
    metrics = rowMetrics(tracks[0], GRID_GAP_PX, labelRowPx(labels), cellAspect);
    if (rowHeight !== metrics.rowHeight) rowHeight = metrics.rowHeight;
  }

  /** Where the visible part is relative to the first row, or `null` while
   *  there is nothing to measure. The grid's top is measured every time:
   *  the toolbar, an error line and banners sit above it in the same
   *  scroll container. */
  function measure(): Geometry | null {
    const grid = el;
    if (!grid || !metrics) return null;
    const rect = grid.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return null;
    let viewTop = 0;
    let viewportHeight = window.innerHeight;
    if (scroller) {
      viewTop = scroller.getBoundingClientRect().top + scroller.clientTop;
      viewportHeight = scroller.clientHeight;
    }
    if (!(viewportHeight > 0)) return null;
    return {
      offset: viewTop - (rect.top + PAD_TOP_PX),
      viewportHeight,
      pitch: metrics.pitch,
      columns: trackCount,
    };
  }

  function sameRange(a: WindowRange, b: WindowRange): boolean {
    return a.start === b.start && a.end === b.end && a.padTop === b.padTop && a.padBottom === b.padBottom;
  }

  /** Re-decides the window. Writes state only when the rendered range would
   *  change, so a scroll within the buffer costs two rect reads. */
  function update(): void {
    const next = measure();
    if (next === null) return;
    const range = windowRange({ ...next, count: items.length, bufferRows });
    if (geometry !== null && sameRange(range, shown)) return;
    geometry = next;
  }

  $effect(() => {
    const grid = el;
    if (!grid) return;
    scroller = scrollParent(grid);
    const target: Element | Window = scroller ?? window;
    const onScroll = () => update();
    // Passive: this handler never calls `preventDefault`.
    target.addEventListener('scroll', onScroll, { passive: true });
    // The grid's width decides the columns and the row height; the
    // container's height decides how many rows are visible. Both change
    // when a hidden view is shown again.
    const resize =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => {
            measureTracks();
            update();
          });
    resize?.observe(grid);
    if (scroller) resize?.observe(scroller);
    untrack(() => {
      measureTracks();
      update();
    });
    return () => {
      target.removeEventListener('scroll', onScroll);
      resize?.disconnect();
      scroller = null;
    };
  });

  $effect(() => {
    // A new card size changes the template but not the grid's own box, so
    // the ResizeObserver above never hears of it. A new label mode changes
    // the row height the same way.
    void size;
    void labels;
    void cellAspect;
    untrack(() => {
      measureTracks();
      update();
    });
  });

  /** The number of columns the browser resolved, for arrow-key movement. */
  export function columns(): number {
    return columnsOf(el);
  }

  /** The grid element. Its children are the rendered cards only. */
  export function element(): HTMLElement | null {
    return el;
  }

  /** The rendered slice of `items`. Reactive: an effect that reads it runs
   *  again when the window moves. */
  export function range(): WindowRange {
    return shown;
  }

  /**
   * Brings card `index` into view: a smooth `nearest` scroll when it is
   * rendered (keyboard movement goes a row at a time, and the buffer rows
   * are rendered), otherwise an instant jump to its row, which renders it.
   */
  export function scrollToIndex(index: number): void {
    const grid = el;
    if (!grid) return;
    if (index >= shown.start && index < shown.end) {
      grid.children[index - shown.start]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      return;
    }
    const at = measure();
    if (at === null || metrics === null) return;
    const current = scroller ? scroller.scrollTop : window.scrollY;
    const top = scrollTopForIndex({
      index,
      columns: at.columns,
      metrics,
      rowsTop: current - at.offset,
      scrollTop: current,
      viewportHeight: at.viewportHeight,
    });
    if (scroller) scroller.scrollTop = top;
    else window.scrollTo(0, top);
    update();
  }
</script>

<!-- D-UI-7: grids may run to the full window width, capped at 1920px. They
     deliberately do NOT take `.view-content` (1100px), which is for lists.
     Windowed (`cards/window.ts`): only the rows near the visible part are
     rendered, and the grid's padding stands in for the rest, so its height
     and the scrollbar match the whole list. Padding, not spacer elements:
     every child of the grid is a card. -->
<div
  data-testid={gridId}
  class="grid"
  bind:this={el}
  style="--template: {gridTemplate(size)}; --cell-aspect: {cellAspect}; --gap: {GRID_GAP_PX}px; --pad-top: {PAD_TOP_PX +
    shown.padTop}px; --pad-bottom: {PAD_BOTTOM_PX + shown.padBottom}px; --row-h: {rowHeight === null
    ? 'auto'
    : `${rowHeight}px`}"
>
  {#each visible as item, j (key(item))}
    {@render card(item, shown.start + j)}
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: var(--template);
    grid-auto-rows: var(--row-h);
    gap: var(--gap);
    /* A little side room so a lifted card's shadow is not clipped by the
       scroll container; the cards line up with the header panel above. */
    padding: var(--pad-top) 4px var(--pad-bottom);
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
    /* The window changes the padding and the cards above the visible part
       on its own; the browser's scroll anchoring would "correct" the scroll
       position for that and fight it. */
    overflow-anchor: none;
  }
</style>
