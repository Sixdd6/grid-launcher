<script lang="ts">
  import Image from './Image.svelte';
  import Icon from './Icon.svelte';
  import { cardBadges, UPDATE_TAG_TEXT } from './cards/badges';
  import type { CardLabels } from './cards/labels';
  import { cardPrimaryLabel, MISSING_BADGE_TEXT } from './library/missing';
  import { ACTION_BAR_HEIGHT_PX, LABEL_GAP_PX, LABEL_HEIGHT_PX, labelRowPx } from './cards/size';
  import type { Rect } from './cards/layout';
  import { recordShape } from './stores/coverShapes.svelte';

  let {
    testId,
    index,
    badgeId,
    title,
    platform,
    coverUrl,
    installed,
    missing = false,
    updateLabel,
    cloudPlatforms,
    labels,
    rect,
    showPlatform = true,
    focused,
    onOpen,
    onPrimary,
    onCloud,
    onHoverStart,
    onHoverEnd,
  }: {
    testId: string;
    /** The card's position in the view's whole list, as `data-index`: the
     *  windowed grid renders a slice, so the background warmer cannot use
     *  the card's position among the grid's children (`visibleWarm.ts`). */
    index: number;
    badgeId: string | number;
    title: string;
    platform: string;
    coverUrl: string | null;
    installed: boolean;
    /** 9c: an installed game whose files are not on disk (Library only). */
    missing?: boolean;
    updateLabel: string | null;
    cloudPlatforms: ReadonlySet<string>;
    /** The "Game labels" setting: a label always under the cover, or only on
     *  hover and keyboard focus. */
    labels: CardLabels;
    /** The cover's rectangle in the grid (`cards/layout.ts`): the card is
     *  the size of its artwork. */
    rect: Rect;
    /** The platform chip, for grids that mix platforms (Library). A
     *  single-platform grid (Server) leaves it off: every card would say the
     *  same thing over its cover. */
    showPlatform?: boolean;
    focused: boolean;
    onOpen: () => void;
    onPrimary: () => void;
    onCloud: () => void;
    onHoverStart: () => void;
    onHoverEnd: () => void;
  } = $props();

  let badges = $derived(cardBadges({ platform, installed, updateLabel, cloudPlatforms }));

  /**
   * Every overlay control stops the click here: the card root's own handler
   * opens Details, and without this an action button would open Details as
   * well as doing its own job.
   */
  function act(handler: () => void) {
    return (e: MouseEvent) => {
      e.stopPropagation();
      handler();
    };
  }
</script>

<!-- A card is the size of its artwork (look-and-feel plan 2026-10-30): no
     frame and no backdrop. The grid lays it out (`cards/layout.ts`) from the
     cover's shape, which the card records once its image has loaded. -->
<div
  data-testid={testId}
  data-index={index}
  class="card"
  class:hover-labels={labels === 'hover'}
  class:focused
  onclick={onOpen}
  onmouseenter={onHoverStart}
  onmouseleave={onHoverEnd}
  role="presentation"
  style="--x: {rect.x}px; --y: {rect.y}px; --w: {rect.w}px; --cover-h: {rect.h}px; --card-h: {rect.h + labelRowPx(labels)}px; --label-h: {LABEL_HEIGHT_PX}px; --label-gap: {LABEL_GAP_PX}px; --bar-h: {ACTION_BAR_HEIGHT_PX}px"
>
    <div class="cover">
      <Image url={coverUrl} alt={title} placeholder="No cover" onmeasure={(w, h) => recordShape(coverUrl, w, h)} />

      <!-- Top-left tags stack in one column so MISSING and UPDATE never cover
           each other (a missing game can still have a newer server version). -->
      {#if missing || badges.update}
        <div class="tag-stack">
          {#if missing}
            <span data-testid={`library-card-missing-${badgeId}`} class="tag missing" title="Files not found">{MISSING_BADGE_TEXT}</span>
          {/if}
          {#if badges.update}
            <span data-testid={`library-update-badge-${badgeId}`} class="tag update">{UPDATE_TAG_TEXT}</span>
          {/if}
        </div>
      {/if}
      {#if badges.installed}
        <span data-testid={`installed-badge-${badgeId}`} class="dot" role="img" aria-label="Installed" title="Installed"></span>
      {/if}
      {#if showPlatform && badges.platform}
        <span data-testid={`card-platform-${badgeId}`} class="tag platform corner">{badges.platform}</span>
      {/if}
      {#if badges.cloud}
        <span data-testid={`card-cloud-badge-${badgeId}`} class="cloud-badge corner" role="img" aria-label="Cloud saves enabled" title="Cloud saves enabled"><Icon name="cloud" size={14} /></span>
      {/if}

      <!-- The shade never takes a click: a click on the cover above the bar
           falls through to `onOpen` (see `ACTION_BAR_HEIGHT_PX`). -->
      <div class="shade" aria-hidden="true"></div>

      <!-- The bar's controls stay in the Tab order: `:focus-within` then
           reveals it, so a keyboard user reaches Play/Install, Cloud sync
           and Details. The card ROOT is not focusable — the views drive
           selection with the `focused` class instead. -->
      <div class="bar">
        <button data-testid={`card-primary-${badgeId}`} class="primary" onclick={act(onPrimary)} tabindex="0">
          {cardPrimaryLabel(installed, missing)}
        </button>
        <button
          data-testid={`card-cloud-${badgeId}`}
          class="round"
          onclick={act(onCloud)}
          tabindex="0"
          aria-label="Cloud sync"
          title="Cloud sync"
        >
          <Icon name="cloud" size={15} />
        </button>
        <button
          data-testid={`card-details-${badgeId}`}
          class="round"
          onclick={act(onOpen)}
          tabindex="0"
          aria-label="Details"
          title="Details"
        >
          <Icon name="more" size={15} />
        </button>
      </div>
    </div>

  <span class="label" title={title}>{title}</span>
</div>

<style>
  /* Placed by the grid: its padding (`--pad-*`, CardGrid) plus the
     layout's rectangle. */
  .card {
    position: absolute;
    left: calc(var(--pad-side, 0px) + var(--x));
    top: calc(var(--pad-top, 0px) + var(--y));
    width: var(--w);
    height: var(--card-h);
    display: flex;
    flex-direction: column;
    gap: var(--label-gap);
    cursor: pointer;
    /* No content-visibility: with real covers, laying out and painting each
       row as it scrolled in cost more per frame (64 ms) than painting every
       card once at open (+100 ms for 300 cards). What bounds the per-frame
       cost now is `CardGrid`'s window: only the rows near the visible part
       are in the DOM, so a 2000-game platform costs what a screenful does
       (2026-10-20, `cards/layout.ts`). */
  }

  .card.hover-labels {
    gap: 0;
  }

  /* Above its neighbours while hovered or focused, so the lifted cover and
     a hover label are never painted under the next card. */
  .card:hover,
  .card.focused,
  .card:focus-within {
    z-index: 2;
  }

  /* The cover is exactly the rectangle the layout gave it, which is the
     art's own shape once known. Until then (portrait) the image is
     contained, so nothing is cropped while the grid catches up. */
  .cover {
    position: relative;
    flex: none;
    width: 100%;
    height: var(--cover-h);
    border-radius: var(--r-card);
    overflow: hidden;
    background: var(--overlay);
    box-shadow: var(--shadow-card);
    transition:
      transform var(--m-fast) ease,
      box-shadow var(--m-fast) ease;
  }

  .cover :global(img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .cover :global(.skeleton),
  .cover :global(.placeholder) {
    width: 100%;
    height: 100%;
  }

  /* D-UI-9: hover and keyboard focus look the same — the cover lifts, and
     only then takes the deep shadow: a wide blurred shadow on every cover
     was the costliest thing in a scrolled frame at 4K (perf probe A/B,
     2026-10-30). */
  .card:hover .cover,
  .card.focused .cover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-lift);
  }

  .card.focused .cover {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  .shade {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--m-fast) ease;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.82), rgba(0, 0, 0, 0) 55%);
  }

  /* Hidden with opacity, NOT `visibility: hidden`: WebDriver refuses to
     click a `visibility: hidden` element, and a driver click hovers the
     card first, which is exactly when these must become clickable.
     `pointer-events: none` keeps them inert until then. */
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--bar-h);
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px;
    box-sizing: border-box;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--m-fast) ease;
  }

  .card:hover .shade,
  .card:focus-within .shade,
  .card:hover .bar,
  .card:focus-within .bar {
    opacity: 1;
  }

  .card:hover .bar,
  .card:focus-within .bar {
    pointer-events: auto;
  }

  /* The corner badges give way to the bar while it shows. */
  .card:hover .corner,
  .card:focus-within .corner {
    opacity: 0;
  }

  .primary {
    flex: 1 1 auto;
    min-width: 0;
    height: 26px;
    padding: 0 10px;
    font: inherit;
    font-size: 11px;
    font-weight: 600;
    border: none;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .primary:hover {
    background: var(--primary-hover);
  }

  .round {
    flex: none;
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    cursor: pointer;
  }

  .round:hover {
    background: var(--primary);
    color: var(--on-primary);
  }

  /* "Always visible": a small label under the cover, so the title
     reads on any art without a panel behind the whole grid. */
  .label {
    align-self: center;
    max-width: 100%;
    height: var(--label-h);
    line-height: var(--label-h);
    padding: 0 9px;
    box-sizing: border-box;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    /* Nearly opaque instead of blurred: a backdrop blur per label re-ran on
       every scrolled frame (perf probe A/B, 2026-10-30). */
    background: var(--label);
    font-size: 12px;
    color: var(--text-h);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* "Visible on hover": the label floats just below the cover, over the gap,
     on an opaque overlay, and only while the card is hovered or focused. */
  .hover-labels .label {
    position: absolute;
    top: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
    width: max-content;
    max-width: calc(100% + 32px);
    background: var(--overlay);
    box-shadow: var(--shadow-lift);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--m-fast) ease;
  }

  .hover-labels:hover .label,
  .hover-labels.focused .label,
  .hover-labels:focus-within .label {
    opacity: 1;
  }

  .tag,
  .cloud-badge {
    transition: opacity var(--m-fast) ease;
  }

  .tag {
    padding: 2px 6px;
    border-radius: var(--r-chip);
    font-size: 10px;
    font-weight: 600;
    line-height: 1.4;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    max-width: calc(100% - 12px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* D-UI-9 placements. The top-right corner belongs to the installed dot,
     so the stack stops short of it. */
  .tag-stack {
    position: absolute;
    top: 6px;
    left: 6px;
    max-width: calc(100% - 28px);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }

  .tag-stack .tag {
    max-width: 100%;
  }

  .tag.update {
    background: var(--warning);
    color: #1a1a12;
    letter-spacing: 0.06em;
  }

  .tag.platform {
    position: absolute;
    bottom: 6px;
    left: 6px;
  }

  /* 9c: first in the stack (the more urgent state). Overlay tags sit on
     cover art, not on a themed surface, so the fill is the fixed light-theme
     danger red: white on #c62828 is 5.6:1 in both themes, where the dark
     theme's lighter --danger would drop white text to 3.2:1. The faint white
     ring keeps it readable on red covers. */
  .tag.missing {
    background: #c62828;
    color: #fff;
    letter-spacing: 0.06em;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.7);
  }

  .dot {
    position: absolute;
    top: 7px;
    right: 7px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.6);
  }

  .cloud-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    padding: 3px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.7);
    color: var(--info);
  }
</style>
