<script lang="ts" generics="T">
  // A grid of covers (items 26–27, 88–90): 10rem columns filling the width,
  // inside a scroll area that reaches 1.2rem past the header on both sides.
  // Moving off its edge reaches only what sits straight past that edge; it
  // is entered where it was left, else at its first cover.
  //
  // Given `items`, it is windowed (risk R8): only the rows near the view and
  // the focused cover's row are rendered, and a spacer above and below
  // stands in for the rest, so the scroll area is as tall as the whole list.
  // Inside it, moves are index arithmetic (nav.svelte.ts over `focus/grid.ts`
  // `gridMove`); only leaving it by an edge goes back to the spatial engine.
  // `key` gives each cover's `data-k` (focus memory finds a cover that is
  // not rendered by it); `listKey` names the list: a new one (another rail
  // row) starts at the top. `children` follow the covers (an empty state).
  import { flushSync, onMount, untrack, type Snippet } from 'svelte';
  import { gridColumns, gridWindow, type GridWindow } from '../focus/grid';
  import { registerWindow } from '../focus/windows';

  let {
    id,
    scroll,
    items = undefined,
    key = undefined,
    cell = undefined,
    listKey = undefined,
    testId = undefined,
    children = undefined,
  }: {
    id: string;
    scroll: string;
    items?: readonly T[];
    key?: (item: T) => string;
    cell?: Snippet<[T, number]>;
    listKey?: unknown;
    testId?: string;
    children?: Snippet;
  } = $props();

  let scrollEl = $state<HTMLElement | null>(null);
  let gridEl = $state<HTMLElement | null>(null);
  let cols = $state(1);
  let pitch = $state(0);
  let rowGap = $state(0);
  let focusIndex = $state<number | null>(null);
  let win = $state<GridWindow>({ start: 0, end: 0, rowsBefore: 0, rowsAfter: 0 });

  let count = $derived(items?.length ?? 0);
  let shown = $derived(items ? items.slice(win.start, win.end) : []);

  function columnsNow(grid: HTMLElement): number {
    const tracks = getComputedStyle(grid).gridTemplateColumns.split(' ').filter((t) => t.endsWith('px')).length;
    if (tracks > 0) return tracks;
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    const cs = getComputedStyle(grid);
    const width = grid.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    return gridColumns(width, 10 * rem, parseFloat(cs.columnGap) || 1.2 * rem);
  }

  /** Measures the grid and its scroll position, and renders what they show. */
  function sync(): void {
    const n = items?.length ?? 0;
    const g = gridEl;
    const s = scrollEl;
    let next: GridWindow;
    if (!g || !s) {
      next = gridWindow({ count: n, columns: 1, pitch: 0, top: 0, bottom: 0, focus: focusIndex });
    } else {
      const cs = getComputedStyle(g);
      const c = columnsNow(g);
      const gap = parseFloat(cs.rowGap) || 0;
      // The exact pitch, from two rows when both are drawn (a rounded
      // height would add up to a visible drift over hundreds of rows).
      const cells = g.querySelectorAll<HTMLElement>('[data-i] [data-f]');
      const first = cells[0];
      const below = cells[c];
      const p = first
        ? below
          ? below.getBoundingClientRect().top - first.getBoundingClientRect().top
          : first.getBoundingClientRect().height + gap
        : 0;
      if (c !== cols) cols = c;
      if (gap !== rowGap) rowGap = gap;
      if (p > 0 && p !== pitch) pitch = p;
      const usePitch = p > 0 ? p : pitch;
      const contentTop = g.getBoundingClientRect().top - s.getBoundingClientRect().top + s.scrollTop + parseFloat(cs.paddingTop);
      const top = s.scrollTop - contentTop;
      next = gridWindow({ count: n, columns: c, pitch: usePitch, top, bottom: top + s.clientHeight, focus: focusIndex });
    }
    const w = win;
    if (w.start !== next.start || w.end !== next.end || w.rowsBefore !== next.rowsBefore || w.rowsAfter !== next.rowsAfter) win = next;
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
      if (scrollEl) scrollEl.scrollTop = 0;
      sync();
    });
  });

  onMount(() => {
    const s = scrollEl;
    const g = gridEl;
    if (!s || !g || !items) return;
    const onScroll = () => sync();
    s.addEventListener('scroll', onScroll, { passive: true });
    const ro = new ResizeObserver(() => sync());
    ro.observe(s);
    ro.observe(g);
    const drop = registerWindow(g, {
      count: () => items?.length ?? 0,
      columns: () => {
        sync();
        return cols;
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
      s.removeEventListener('scroll', onScroll);
      ro.disconnect();
      drop();
    };
  });

  /** A spacer's height for `rows` left-out rows (the grid's row gap adds the last gap). */
  const spacer = (rows: number) => `grid-column: 1 / -1; height: ${Math.max(0, rows * pitch - rowGap)}px`;
</script>

<div class="scroll" class:windowed={!!items} data-scroll={scroll} bind:this={scrollEl}>
  {#if items}
    <div class="grid" {id} data-nav="grid" data-enter="first" data-rail-target data-window data-count={count} data-testid={testId} bind:this={gridEl}>
      {#if win.rowsBefore > 0 && pitch > 0}<div class="gspace" aria-hidden="true" style={spacer(win.rowsBefore)}></div>{/if}
      {#each shown as item, j (key ? key(item) : win.start + j)}
        <div class="gcell" data-i={win.start + j}>{@render cell?.(item, win.start + j)}</div>
      {/each}
      {#if win.rowsAfter > 0 && pitch > 0}<div class="gspace" aria-hidden="true" style={spacer(win.rowsAfter)}></div>{/if}
      {@render children?.()}
    </div>
  {:else}
    <div class="grid" {id} data-nav="grid" data-enter="first" data-rail-target data-testid={testId} bind:this={gridEl}>
      {@render children?.()}
    </div>
  {/if}
</div>
