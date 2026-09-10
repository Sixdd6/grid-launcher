<script lang="ts">
  import Image from './Image.svelte';
  import Icon from './Icon.svelte';
  import { cardBadges, UPDATE_TAG_TEXT } from './cards/badges';
  import {
    ACTION_ROW_HEIGHT_PX,
    CARD_COVER_RATIO,
    CARD_GAP_PX,
    PRIMARY_CENTRE_FRACTION,
    PRIMARY_HEIGHT_PX,
    TITLE_ROW_HEIGHT_PX,
  } from './cards/size';

  let {
    testId,
    badgeId,
    title,
    platform,
    coverUrl,
    installed,
    updateLabel,
    cloudPlatforms,
    focused,
    onOpen,
    onPrimary,
    onCloud,
    onHoverStart,
    onHoverEnd,
  }: {
    testId: string;
    badgeId: string | number;
    title: string;
    platform: string;
    coverUrl: string | null;
    installed: boolean;
    updateLabel: string | null;
    cloudPlatforms: ReadonlySet<string>;
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

<div
  data-testid={testId}
  class="card"
  class:focused
  onclick={onOpen}
  onmouseenter={onHoverStart}
  onmouseleave={onHoverEnd}
  role="presentation"
  style="--cover-ratio: {CARD_COVER_RATIO}; --title-h: {TITLE_ROW_HEIGHT_PX}px; --card-gap: {CARD_GAP_PX}px; --primary-y: {PRIMARY_CENTRE_FRACTION * 100}%; --primary-h: {PRIMARY_HEIGHT_PX}px; --action-h: {ACTION_ROW_HEIGHT_PX}px"
>
  <div class="cover">
    <Image url={coverUrl} alt={title} placeholder="No cover" backdrop />

    {#if badges.update}
      <span data-testid={`library-update-badge-${badgeId}`} class="tag update">{UPDATE_TAG_TEXT}</span>
    {/if}
    {#if badges.installed}
      <span data-testid={`installed-badge-${badgeId}`} class="dot" role="img" aria-label="Installed" title="Installed"></span>
    {/if}
    {#if badges.platform}
      <span data-testid={`card-platform-${badgeId}`} class="tag platform">{badges.platform}</span>
    {/if}
    {#if badges.cloud}
      <span data-testid={`card-cloud-badge-${badgeId}`} class="cloud-badge" role="img" aria-label="Cloud saves enabled" title="Cloud saves enabled"><Icon name="cloud" size={14} /></span>
    {/if}

    <!-- The gradient itself never takes a click: the band around the card
         root's centre must fall through to `onOpen` (see size.ts). -->
    <div class="overlay" aria-hidden="true"></div>

    <button
      data-testid={`card-primary-${badgeId}`}
      class="primary"
      onclick={act(onPrimary)}
      tabindex="0"
    >
      {installed ? 'Play' : 'Install'}
    </button>

    <!-- The four overlay controls stay in the Tab order: `:focus-within`
         then reveals the overlay, so a keyboard or gamepad user reaches
         Play/Install, Cloud sync and More. The card ROOT is not focusable —
         the views drive selection with the `focused` class instead. -->
    <div class="actions">
      <button data-testid={`card-details-${badgeId}`} onclick={act(onOpen)} tabindex="0">Details</button>
      <button data-testid={`card-cloud-${badgeId}`} onclick={act(onCloud)} tabindex="0">Cloud sync</button>
      <button data-testid={`card-more-${badgeId}`} onclick={act(onOpen)} tabindex="0">More</button>
    </div>
  </div>

  <span class="title">{title}</span>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--card-gap);
    cursor: pointer;
    transform: scale(1);
    transition: transform var(--m-fast) cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
    /* No content-visibility: with real covers, laying out and painting each
       row as it scrolled in cost more per frame (64 ms) than painting every
       card once at open (+100 ms for 300 cards). */
  }

  /* D-UI-9: hover scales 1.05. Focus (gamepad/arrow keys) uses the same
     scale so the two selection models look identical. */
  .card:hover,
  .card.focused {
    transform: scale(1.05);
    z-index: 1;
  }

  /* Drawn INSIDE the cover, with a negative offset: the ring then follows
     the cover's rounded corners and stays within the card's own box, so a
     focused card cannot paint over the gap to its neighbours while it is
     scaled up. (It began as a workaround for `content-visibility`'s paint
     containment, which round 8 removed; the inset ring is kept because it
     is the look the design shows.) */
  .card.focused .cover {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .cover {
    position: relative;
    aspect-ratio: var(--cover-ratio);
    border-radius: var(--r-card);
    overflow: hidden;
    background: var(--surface-2);
  }

  /* The user's review choice ("option B"): the frame stays 3:4 so rows
     stay even, the whole cover fits inside it, and a blurred, dimmed copy
     of the same cover fills the letterbox for square (PS1) and wide
     (Genesis) art instead of cropping their sides. */
  .cover :global(img) {
    position: relative;
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  /* Round 8: the blurred copy is rasterised at a QUARTER of the cover's
     size and scaled up by 4 — the upscale supplies most of the softening,
     so a 2.5px blur at quarter size reads like the old 10px blur at full
     size while touching one sixteenth of the pixels. Measured on a
     302-card platform with real covers: 64 → 28 ms per scroll frame with
     the content-visibility change above, and no frame over 50 ms. */
  .cover :global(img.backdrop) {
    position: absolute;
    top: -12px;
    left: -12px;
    width: calc((100% + 24px) / 4);
    height: calc((100% + 24px) / 4);
    transform: scale(4);
    transform-origin: top left;
    object-fit: cover;
    filter: blur(2.5px) brightness(0.45);
    pointer-events: none;
  }

  .title {
    height: var(--title-h);
    line-height: var(--title-h);
    font-size: 12px;
    color: var(--text-h);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--m-fast) ease;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.85), rgba(0, 0, 0, 0.15) 55%, rgba(0, 0, 0, 0.35));
  }

  .card:hover .overlay,
  .card:focus-within .overlay {
    opacity: 1;
  }

  /* Hidden with opacity, NOT `visibility: hidden`: WebDriver refuses to
     click a `visibility: hidden` element, and a driver click hovers the
     card first, which is exactly when these must become clickable.
     `pointer-events: none` keeps them inert until then. */
  .primary,
  .actions {
    position: absolute;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--m-fast) ease;
  }

  .card:hover .primary,
  .card:hover .actions,
  .card:focus-within .primary,
  .card:focus-within .actions {
    opacity: 1;
    pointer-events: auto;
  }

  .primary {
    top: var(--primary-y);
    left: 50%;
    transform: translate(-50%, -50%);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    height: var(--primary-h);
    padding: 0 18px;
    border: none;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .primary:hover {
    background: var(--primary-hover);
  }

  .actions {
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--action-h);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 0 4px;
    box-sizing: border-box;
  }

  .actions button {
    font: inherit;
    font-size: 10px;
    padding: 3px 6px;
    border: none;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions button:hover {
    background: var(--primary);
  }

  .tag {
    position: absolute;
    padding: 2px 6px;
    border-radius: var(--r-chip);
    font-size: 10px;
    font-weight: 600;
    line-height: 1.4;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    max-width: calc(100% - 12px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* D-UI-9 placements. */
  .tag.update {
    top: 6px;
    left: 6px;
    background: var(--warning);
    color: #1a1a12;
    letter-spacing: 0.06em;
  }

  .tag.platform {
    bottom: 6px;
    left: 6px;
  }

  .dot {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.55);
  }

  .cloud-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    padding: 3px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.65);
    color: var(--info);
  }
</style>
