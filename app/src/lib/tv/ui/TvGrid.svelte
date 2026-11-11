<script lang="ts" generics="T">
  // A grid of covers (items 26–27, 88–90), each the shape of its art, laid
  // out as the desktop lays out its cards (`cards/layout.ts`): Equal height
  // rows or Equal width columns, the "Game card display" setting both modes
  // share. Sizes are in rem, so the grid scales with the TV window: rows
  // around a 10rem-wide portrait cover's height, columns at least 10rem.
  // Moving off its edge reaches only what sits straight past that edge; it
  // is entered where it was left, else at its first cover.
  //
  // Given `items`, it is windowed (risk R8): every cover's rectangle is
  // known, only the covers near the view and the focused one are rendered,
  // and the grid is as tall as the whole layout. Inside it, moves are worked
  // out on the rectangles (nav.svelte.ts over `focus/grid.ts` `layoutMove`);
  // only leaving it by an edge goes back to the spatial engine. `key` gives
  // each cover's `data-k` (focus memory finds a cover that is not rendered
  // by it); `cover` its art, whose shape sizes it; `listKey` names the list:
  // a new one (another rail row) starts at the top. `children` follow the
  // covers (an empty state).
  //
  // `scroll` names the grid's own scroll area. Without it the grid sits in
  // a page that scrolls (Search everything) and uses that.
  import { flushSync, onMount, untrack, type Snippet } from 'svelte';
  import { columnsLayout, rowsLayout, visibleRange, type Layout } from '../../cards/layout';
  import { CARD_CELL_ASPECT } from '../../cards/size';
  import { shapeOf } from '../../stores/coverShapes.svelte';
  import { uiSettings } from '../../stores/uiSettings.svelte';
  import { scrollParent } from '../../visibleWarm';
  import { INITIAL_WINDOW, layoutMove, windowIndexes } from '../focus/grid';
  import { registerWindow } from '../focus/windows';
  import { revealFocus } from '../nav.svelte';

  let {
    id,
    scroll = undefined,
    items = undefined,
    key = undefined,
    cover = undefined,
    cell = undefined,
    listKey = undefined,
    testId = undefined,
    children = undefined,
  }: {
    id: string;
    scroll?: string;
    items?: readonly T[];
    key?: (item: T) => string;
    /** The item's cover URL: its shape sizes the cover. */
    cover?: (item: T) => string | null;
    cell?: Snippet<[T, number]>;
    listKey?: unknown;
    testId?: string;
    children?: Snippet;
  } = $props();

  /** A cover's width at the portrait shape, and the gaps, in rem (tv.css). */
  const COVER_REM = 10;
  const GAP_REM = 1.2;
  const ROW_GAP_REM = 1.6;
  /** The title under a cover: the cover's .5rem gap and one .9rem line at
   *  line-height 1.4 (tv.css `.cv-l`). */
  const LABEL_REM = 0.5 + 0.9 * 1.4;
  /** How far past the visible part covers are rendered, in rem. */
  const BUFFER_REM = 30;

  let scrollEl = $state<HTMLElement | null>(null);
  let gridEl = $state<HTMLElement | null>(null);
  /** The box the covers are placed in (inside the grid's padding). */
  let boxEl = $state<HTMLElement | null>(null);
  /** The root font size, in px: every size here is in rem. */
  let rem = $state(16);
  /** The content width, once measured. */
  let width = $state<number | null>(null);
  /** The visible band, in px from the layout's top; null until measured. */
  let band = $state.raw<{ top: number; bottom: number } | null>(null);
  let focusIndex = $state<number | null>(null);
  // Plain: read on every scroll event.
  let scroller: Element | null = null;

  let count = $derived(items?.length ?? 0);
  let shapes = $derived(items ? items.map((it) => shapeOf(cover?.(it) ?? null)) : []);
  let layout: Layout = $derived.by(() => {
    const w = width ?? 1280;
    const common = { shapes, width: w, labelRow: LABEL_REM * rem, gap: GAP_REM * rem, rowGap: ROW_GAP_REM * rem };
    return uiSettings.cardDisplay === 'columns'
      ? columnsLayout({ ...common, minColumn: COVER_REM * rem })
      : rowsLayout({ ...common, target: (COVER_REM / CARD_CELL_ASPECT) * rem });
  });
  let shown = $derived.by(() => {
    const range =
      band === null
        ? { start: 0, end: Math.min(count, INITIAL_WINDOW) }
        : visibleRange(layout, LABEL_REM * rem, band.top - BUFFER_REM * rem, band.bottom + BUFFER_REM * rem);
    return windowIndexes(range, focusIndex, count);
  });

  /** Measures the grid and its scroll position, and renders what they show. */
  function sync(): void {
    const el = boxEl;
    if (!el) return;
    const box = el.getBoundingClientRect();
    // A hidden grid measures nothing; the last measurement still holds.
    if (box.width === 0 && box.height === 0 && el.clientWidth === 0) return;
    const r = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    if (r !== rem) rem = r;
    const w = Math.floor(el.clientWidth);
    if (w > 0 && w !== width) width = w;
    const s = scroller;
    const viewTop = s ? s.getBoundingClientRect().top : 0;
    const viewHeight = s ? s.clientHeight : window.innerHeight;
    if (!(viewHeight > 0)) return;
    const top = viewTop - box.top;
    // A new band only when it moved far enough to matter: a scroll inside
    // the buffer costs two rect reads and no render.
    const b = band;
    if (b !== null && Math.abs(b.top - top) < (BUFFER_REM * rem) / 4 && b.bottom - b.top === viewHeight) return;
    band = { top, bottom: top + viewHeight };
  }

  // The list changed (a search, a refresh): render for it now.
  $effect(() => {
    void items;
    untrack(() => {
      if (focusIndex !== null && focusIndex >= count) focusIndex = count > 0 ? count - 1 : null;
      sync();
    });
  });

  // Another list: from its top, with no cover held.
  let seenList: unknown = undefined;
  let listSeen = false;
  $effect(() => {
    const k = listKey;
    if (!listSeen) {
      listSeen = true;
      seenList = k;
      return;
    }
    if (k === seenList) return;
    seenList = k;
    untrack(() => {
      focusIndex = null;
      if (scroller) scroller.scrollTop = 0;
      sync();
    });
  });

  onMount(() => {
    const g = gridEl;
    if (!g || !items) return;
    scroller = scrollEl ?? scrollParent(g);
    const target: Element | Window = scroller ?? window;
    const onScroll = () => sync();
    target.addEventListener('scroll', onScroll, { passive: true });
    const ro = new ResizeObserver(() => sync());
    ro.observe(g);
    if (boxEl) ro.observe(boxEl);
    if (scroller) ro.observe(scroller);
    const drop = registerWindow(g, {
      count: () => items?.length ?? 0,
      move: (i, dir) => {
        sync();
        return layoutMove(layout.rects, i, dir);
      },
      indexOfKey: (k) => (items && key ? items.findIndex((it) => key(it) === k) : -1),
      ensure: (i) => {
        focusIndex = i;
        sync();
        flushSync();
        return g.querySelector<HTMLElement>(`[data-i="${i}"] [data-f]`);
      },
      focusAt: (i) => {
        if (focusIndex !== i) focusIndex = i;
      },
      sync: () => {
        sync();
        flushSync();
      },
    });
    sync();
    return () => {
      target.removeEventListener('scroll', onScroll);
      ro.disconnect();
      drop();
      scroller = null;
    };
  });

  // A cover that loads reshapes its rectangle, and every cover after it
  // moves: when that moves the focused cover (and focus did not move), the
  // scrolling rules run again for it, so it never ends up off screen.
  let held: { index: number; x: number; y: number } | null = null;
  $effect(() => {
    const i = focusIndex;
    const r = i !== null ? layout.rects[i] : undefined;
    untrack(() => {
      const was = held;
      held = i !== null && r ? { index: i, x: r.x, y: r.y } : null;
      if (!held || !was || was.index !== held.index || (was.x === held.x && was.y === held.y)) return;
      const g = gridEl;
      if (!g || !g.querySelector(`[data-i="${held.index}"] .f`)) return;
      // After the cells have moved.
      requestAnimationFrame(() => revealFocus());
    });
  });

  /** A cover's cell: placed by its rectangle, sized for the cover inside. */
  function place(i: number): string {
    const r = layout.rects[i];
    if (!r) return '';
    return `left: ${r.x}px; top: ${r.y}px; width: ${r.w}px; --cw: ${r.w}px; --ch: ${r.h}px`;
  }
</script>

{#snippet grid()}
  {#if items}
    <div
      class="grid laid"
      {id}
      data-nav="grid"
      data-enter="first"
      data-rail-target
      data-window
      data-count={count}
      data-display={uiSettings.cardDisplay}
      data-testid={testId}
      bind:this={gridEl}
    >
      <div class="gbox" style={`height: ${count > 0 ? layout.height : 0}px`} bind:this={boxEl}>
        {#each shown as i (key && items[i] !== undefined ? key(items[i]) : i)}
          {#if items[i] !== undefined}
            <div class="gcell" data-i={i} style={place(i)}>{@render cell?.(items[i], i)}</div>
          {/if}
        {/each}
      </div>
      {@render children?.()}
    </div>
  {:else}
    <div class="grid" {id} data-nav="grid" data-enter="first" data-rail-target data-testid={testId} bind:this={gridEl}>
      {@render children?.()}
    </div>
  {/if}
{/snippet}

{#if scroll}
  <div class="scroll" class:windowed={!!items} data-scroll={scroll} bind:this={scrollEl}>
    {@render grid()}
  </div>
{:else}
  {@render grid()}
{/if}
