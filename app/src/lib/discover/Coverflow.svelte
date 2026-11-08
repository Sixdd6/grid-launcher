<script lang="ts">
  // New releases: a coverflow. The centre cover flat and lit; the others
  // tilt 52° back, recede and dim, each with a soft reflection. Arrows, a
  // click on a side cover, or ← / → move it (450ms glide); a click on the
  // centre cover, or Enter, opens Details.
  //
  // Each item is one flat layer under the stage's perspective: no
  // preserve-3d, no animated filter and no -webkit-box-reflect, which
  // WebKitGTK's compositor drew flat, dropped or flickered mid-glide. The
  // reflection is a flipped copy of the cover faded by a mask, and the dimming
  // is a dark shade whose opacity eases.
  import type { GameSummary } from '../api';
  import Icon from '../Icon.svelte';
  import Image from '../Image.svelte';
  import { clampIndex, coverPlace, releasedLine, type DiscoverAction } from './feed';

  let {
    games,
    index = $bindable(0),
    labelFor,
    actionFor,
    onOpen,
    onAction,
  }: {
    games: GameSummary[];
    index?: number;
    labelFor: (game: GameSummary) => string;
    actionFor: (game: GameSummary) => DiscoverAction;
    onOpen: (game: GameSummary) => void;
    onAction: (game: GameSummary) => void;
  } = $props();

  let stage = $state<HTMLElement | null>(null);
  let current = $derived(games[clampIndex(index, games.length)] ?? null);

  export function go(next: number) {
    index = clampIndex(next, games.length);
  }

  /** ← / → / Enter, from the coverflow itself or (Server) the window. */
  export function handleKey(e: KeyboardEvent): boolean {
    if (e.key === 'ArrowLeft') go(index - 1);
    else if (e.key === 'ArrowRight') go(index + 1);
    else if (e.key === 'Enter' && current) onOpen(current);
    else return false;
    e.preventDefault();
    return true;
  }

  export function focus() {
    stage?.focus();
  }

  function onCover(i: number) {
    if (i === index) onOpen(games[i]);
    else go(i);
  }
</script>

<div
  data-testid="discover-coverflow"
  class="cf"
  tabindex="0"
  role="group"
  aria-label="New releases"
  aria-roledescription="carousel"
  bind:this={stage}
  onkeydown={(e) => {
    if (handleKey(e)) e.stopPropagation();
  }}
>
  <button
    data-testid="discover-cf-prev"
    class="cf-arrow prev"
    aria-label="Previous"
    tabindex="-1"
    disabled={index <= 0}
    onclick={() => go(index - 1)}
  >
    <Icon name="chevronLeft" size={20} />
  </button>
  <div class="cf-stage">
    {#each games as game, i (game.id)}
      {@const place = coverPlace(i - index)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        data-testid={`discover-cover-${game.id}`}
        data-center={place.center ? 'true' : 'false'}
        class="cf-item"
        class:center={place.center}
        class:gone={place.hidden}
        style:transform={place.transform}
        style:z-index={place.zIndex}
        aria-hidden={place.hidden ? 'true' : 'false'}
        role="presentation"
        onclick={() => onCover(i)}
      >
        <div class="cf-cover">
          <Image url={game.path_cover_large || game.path_cover_small} alt={game.name} placeholder={game.name} class="cf-img" />
          <span class="cf-shade"></span>
        </div>
        <div class="cf-reflection" aria-hidden="true">
          <div class="cf-mirror">
            <Image url={game.path_cover_large || game.path_cover_small} alt="" placeholder="" class="cf-img" />
            <span class="cf-shade"></span>
          </div>
        </div>
      </div>
    {/each}
  </div>
  <button
    data-testid="discover-cf-next"
    class="cf-arrow next"
    aria-label="Next"
    tabindex="-1"
    disabled={index >= games.length - 1}
    onclick={() => go(index + 1)}
  >
    <Icon name="chevronRight" size={20} />
  </button>
</div>

{#if current}
  {@const action = actionFor(current)}
  <div data-testid="discover-cf-caption" class="cf-caption" aria-live="polite">
    <b data-testid="discover-cf-title">{current.name}</b>
    <span class="meta">
      {[labelFor(current), releasedLine(current.first_release_date)].filter((s) => s !== '').join(' · ')}
    </span>
    <button
      data-testid="discover-cf-action"
      class="action"
      disabled={action === 'Installing…' || action === 'Queued'}
      onclick={() => onAction(current)}
    >
      {action}
    </button>
  </div>
{/if}

<style>
  /* `isolation`: the covers' and arrows' z-index rank only inside the
     coverflow, never against the app's overlays. */
  .cf {
    position: relative;
    height: 330px;
    outline: none;
    border-radius: var(--r-row);
    isolation: isolate;
  }

  .cf:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  /* The stage runs on below the coverflow so the reflections can fade out
     under the caption, and clips everything past the sides. A plain
     overflow clip on a flat parent: the items are single 3D-transformed
     layers, so nothing here flattens or splits them. It takes no clicks
     itself (it overlaps the caption); the covers do. */
  .cf-stage {
    position: absolute;
    inset: 0 0 -110px;
    perspective: 1100px;
    overflow: hidden;
    pointer-events: none;
  }

  .cf-item {
    position: absolute;
    left: 50%;
    top: 14px;
    width: 200px;
    margin-left: -100px;
    cursor: pointer;
    pointer-events: auto;
    will-change: transform;
    backface-visibility: hidden;
    transition:
      transform var(--m-glide, 450ms) cubic-bezier(0.2, 0.7, 0.2, 1),
      opacity var(--m-glide, 450ms) ease;
  }

  .cf-item.gone {
    opacity: 0;
    pointer-events: none;
  }

  .cf-cover {
    position: relative;
    aspect-ratio: 3 / 4;
    border-radius: var(--r-card);
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow-card);
    transition: transform 150ms ease;
  }

  .cf-item.center .cf-cover {
    box-shadow: var(--shadow-lift);
  }

  .cf-item.center:hover .cf-cover {
    transform: translateY(-4px);
  }

  .cf-cover :global(.cf-img),
  .cf-mirror :global(.cf-img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* The side covers' dimming (the old brightness(0.62)), on the cover and
     its reflection alike; the centre's shade fades out as it arrives. Its
     colour eases, not its opacity: an opacity transition gets its own
     compositing layer, and one inside the masked reflection is the kind of
     layer WebKitGTK drops mid-glide. A colour repaints inside the item. */
  .cf-shade {
    position: absolute;
    inset: 0;
    background-color: rgba(0, 0, 0, 0.38);
    transition: background-color var(--m-glide, 450ms) ease;
  }

  .cf-item.center .cf-shade {
    background-color: rgba(0, 0, 0, 0);
  }

  /* The reflection: 6px under the cover, as tall as it, strongest at the
     cover's edge and gone before halfway. The mask sits on this unflipped
     box so the gradient runs top to bottom; the copy inside is flipped. */
  .cf-reflection {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    width: 100%;
    aspect-ratio: 3 / 4;
    pointer-events: none;
    -webkit-mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.3), rgba(0, 0, 0, 0.1) 22%, transparent 42%);
    mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.3), rgba(0, 0, 0, 0.1) 22%, transparent 42%);
  }

  .cf-mirror {
    position: relative;
    height: 100%;
    border-radius: var(--r-card);
    overflow: hidden;
    background: var(--surface-2);
    transform: scaleY(-1);
  }

  .cf-arrow {
    position: absolute;
    top: 50%;
    margin-top: -24px;
    z-index: 60;
    width: 36px;
    height: 48px;
    display: grid;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: var(--r-control);
    background: var(--overlay);
    color: var(--text-h);
    box-shadow: var(--shadow-lift);
    cursor: pointer;
  }

  .cf-arrow.prev {
    left: 8px;
  }

  .cf-arrow.next {
    right: 8px;
  }

  .cf-arrow:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .cf-caption {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    text-align: center;
  }

  .cf-caption b {
    color: var(--text-h);
    font-size: 18px;
  }

  .meta {
    color: var(--text-muted);
    font-size: 12px;
  }

  .action {
    margin-top: 6px;
    font: inherit;
    font-size: 13px;
    padding: 6px 18px;
    border: none;
    border-radius: var(--r-control);
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
  }

  .action:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .action:disabled {
    opacity: 0.7;
    cursor: default;
  }

  @media (prefers-reduced-motion: reduce) {
    .cf-item,
    .cf-cover,
    .cf-shade {
      transition: none;
    }
  }
</style>
