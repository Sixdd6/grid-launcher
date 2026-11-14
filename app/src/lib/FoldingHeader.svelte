<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { foldedClearance, nextFolded, unfoldedHeight } from './views/fold';

  let {
    scroller,
    resetKey,
    folded = $bindable(false),
    testId,
    title,
    toolbar,
  }: {
    /** The grid's scroll container. The header floats over its top; this
     *  component gives it `--head-h` (the unfolded height, reserved as its
     *  top padding) and `--head-clear` (where a card brought into view must
     *  sit to clear the folded bar). */
    scroller: HTMLElement | null;
    /** A new value (another platform or rail entry) starts the grid at the
     *  top, unfolded. */
    resetKey: unknown;
    folded?: boolean;
    testId?: string;
    /** The row that folds away: title, counts, chips. */
    title: Snippet;
    /** The row that stays: search and the selects. */
    toolbar: Snippet;
  } = $props();

  let headEl = $state<HTMLElement | null>(null);
  let titleEl = $state<HTMLElement | null>(null);
  let barEl = $state<HTMLElement | null>(null);

  $effect(() => {
    const sc = scroller;
    if (!sc) return;
    const onScroll = () => {
      const next = nextFolded(folded, sc.scrollTop);
      if (next !== folded) folded = next;
    };
    sc.addEventListener('scroll', onScroll, { passive: true });
    untrack(onScroll);
    return () => sc.removeEventListener('scroll', onScroll);
  });

  // Through a `$derived`, so the reset follows the key's VALUE. Read straight
  // from the prop, the effect also tracked whatever the caller's expression
  // reads: Library passes `selected.key`, and `selected` is a new object on
  // every `installed.list` refresh (a store Details refreshes it as it
  // opens), which sent the grid back to the top under the popup.
  let key = $derived(resetKey);
  $effect(() => {
    void key;
    untrack(() => {
      if (scroller) scroller.scrollTop = 0;
      folded = false;
    });
  });

  // Measured from the two rows' own heights, which do not animate, never
  // from the header itself: its height changes all through a fold, and
  // following it would move every card.
  $effect(() => {
    const sc = scroller;
    const head = headEl;
    const t = titleEl;
    const bar = barEl;
    if (!sc || !head || !t || !bar) return;
    const measure = () => {
      const barH = bar.offsetHeight;
      // A hidden view measures zero; the last measurement still holds.
      if (barH === 0) return;
      sc.style.setProperty('--head-h', `${unfoldedHeight(t.offsetHeight, barH)}px`);
      sc.style.setProperty('--head-clear', `${foldedClearance(barH)}px`);
      // Leave the scroller's scrollbar uncovered.
      head.style.right = `${sc.offsetWidth - sc.clientWidth}px`;
    };
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    resize?.observe(t);
    resize?.observe(bar);
    resize?.observe(sc);
    measure();
    return () => resize?.disconnect();
  });
</script>

<div data-testid={testId} class="fold-head panel scrolls" class:folded data-folded={folded} bind:this={headEl}>
  <div class="fold">
    <div class="fold-in" inert={folded}>
      <div class="fold-gap">
        <div class="fold-title" bind:this={titleEl}>{@render title()}</div>
      </div>
    </div>
  </div>
  <div class="fold-bar" bind:this={barEl}>{@render toolbar()}</div>
</div>

<style>
  /* Floats over the top of the scroller (its parent is `position:
     relative`); the scroller reserves the unfolded height, so a fold never
     moves a card. */
  .fold-head {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 3;
    padding: 14px 16px;
    transition:
      padding var(--m-fold) var(--ease-fold),
      background-color var(--m-fold) ease,
      box-shadow var(--m-fold) ease,
      border-color var(--m-fold) ease;
  }

  /* Folded, the bar floats over the covers: the opaque overlay surface, so
     no cover shows through the search field, with a soft shadow and a
     stronger edge. */
  .fold-head.folded {
    padding-top: 8px;
    padding-bottom: 8px;
    background: var(--overlay);
    border-color: var(--line-strong);
    box-shadow: 0 10px 24px rgba(10, 10, 30, 0.22);
  }

  .fold {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows var(--m-fold) var(--ease-fold);
  }

  .folded .fold {
    grid-template-rows: 0fr;
  }

  .fold-in {
    min-height: 0;
    overflow: hidden;
  }

  /* The gap to the toolbar row closes with the title row. */
  .fold-gap {
    padding-bottom: 12px;
  }

  /* The title row lifts and fades as it folds; the chips go a beat earlier.
     Unfolding, the row waits a moment so it appears into the opening space. */
  .fold-title {
    transition:
      opacity 180ms ease 60ms,
      transform var(--m-fold) var(--ease-fold);
  }

  .folded .fold-title {
    opacity: 0;
    transform: translateY(-8px);
    transition:
      opacity 160ms ease,
      transform var(--m-fold) var(--ease-fold);
  }

  .fold-title :global(.chips) {
    transition: opacity 140ms ease 80ms;
  }

  .folded .fold-title :global(.chips) {
    opacity: 0;
    transition: opacity 100ms ease;
  }

  @media (prefers-reduced-motion: reduce) {
    .fold-head,
    .fold-head :global(*) {
      transition: none !important;
    }
  }
</style>
