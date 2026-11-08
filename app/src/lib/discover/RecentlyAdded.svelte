<script lang="ts">
  // Recently added: a large screenshot on the left, a column of small covers
  // on the right. The highlight frame glides to the next row every 10 s
  // (450ms) while the screenshot crossfades (600ms), wrapping at the end; a
  // thin bar inside the frame fills over the 10 s. Pointing at the section
  // pauses it; clicking a row jumps to it; clicking the screenshot or the
  // highlighted row opens Details. The timer is JavaScript (`rotation.ts`),
  // so it runs under reduced motion too, where the animations are off.
  import { tick, untrack } from 'svelte';
  import type { GameSummary } from '../api';
  import Image from '../Image.svelte';
  import Shot from './Shot.svelte';
  import { addedAgo, ROTATE_MS, wrapIndex, type DiscoverAction } from './feed';
  import { createRotation } from './rotation';

  let {
    games,
    resetKey = '',
    labelFor,
    actionFor,
    onOpen,
    onAction,
  }: {
    games: GameSummary[];
    /** A change starts the list at its first row (a platform filter). */
    resetKey?: string;
    labelFor: (game: GameSummary) => string;
    actionFor: (game: GameSummary) => DiscoverAction;
    onOpen: (game: GameSummary) => void;
    onAction: (game: GameSummary) => void;
  } = $props();

  let index = $state(0);
  let paused = $state(false);
  /** Bumped on every move, to restart the bar's fill. */
  let cycle = $state(0);
  /** The two crossfading layers and which one is in front. */
  let layers = $state.raw<[GameSummary | null, GameSummary | null]>([null, null]);
  let front = $state<0 | 1>(0);

  let listEl = $state<HTMLElement | null>(null);
  let rowEls: HTMLElement[] = $state([]);
  let frame = $state<{ top: number; height: number } | null>(null);

  let current = $derived(games[wrapIndex(index, games.length)] ?? null);

  const rotation = createRotation(ROTATE_MS, () => go(index + 1));

  $effect(() => () => rotation.stop());

  // A new list (a reload) keeps the row when it is still there; a new
  // `resetKey` (another platform filter) starts at the first row.
  let lastKey: string | null = null;
  $effect(() => {
    const list = games;
    const key = resetKey;
    untrack(() => {
      if (key !== lastKey || index > list.length - 1) index = 0;
      lastKey = key;
      const game = list[wrapIndex(index, list.length)] ?? null;
      layers = [game, null];
      front = 0;
      cycle += 1;
      rotation.restart();
      void place();
    });
  });

  function go(next: number) {
    if (games.length === 0) return;
    index = wrapIndex(next, games.length);
    const game = games[index];
    const back: 0 | 1 = front === 0 ? 1 : 0;
    const nextLayers: [GameSummary | null, GameSummary | null] = [...layers];
    nextLayers[back] = game;
    layers = nextLayers;
    front = back;
    cycle += 1;
    rotation.restart();
    void place();
  }

  /** Moves the frame onto the active row and keeps the row in view. */
  async function place() {
    await tick();
    const row = rowEls[index];
    if (!row) return;
    frame = { top: row.offsetTop, height: row.offsetHeight };
    if (listEl) {
      const top = Math.max(0, row.offsetTop - (listEl.clientHeight - row.offsetHeight) / 2);
      listEl.scrollTo?.({ top, behavior: 'smooth' });
    }
  }

  function pause() {
    paused = true;
    rotation.pause();
  }

  function resume() {
    paused = false;
    rotation.resume();
  }

  function onRow(i: number) {
    if (i === index) onOpen(games[i]);
    else go(i);
  }

  function onShotKey(e: KeyboardEvent) {
    if ((e.key === 'Enter' || e.key === ' ') && current) {
      e.preventDefault();
      onOpen(current);
    }
  }

  function caption(game: GameSummary): string {
    return [labelFor(game), addedAgo(game.created_at, Date.now())].filter((s) => s !== '').join(' · ');
  }
</script>

<div
  data-testid="discover-recent-list"
  class="ra"
  role="group"
  aria-label="Recently added"
  onpointerenter={pause}
  onpointerleave={resume}
  onfocusin={pause}
  onfocusout={(e) => {
    const next = e.relatedTarget as Node | null;
    if (!next || !(e.currentTarget as HTMLElement).contains(next)) resume();
  }}
>
  <div class="ra-main">
    <div
      data-testid="discover-recent-shot"
      class="ra-shot"
      role="button"
      tabindex="0"
      aria-label={current ? `Open details for ${current.name}` : 'Open details'}
      onclick={() => current && onOpen(current)}
      onkeydown={onShotKey}
    >
      {#each layers as layer, i (i)}
        <div class="ra-layer" class:on={front === i && layer !== null}>
          {#if layer}
            {#key layer.id}
              <Shot game={layer} />
            {/key}
          {/if}
        </div>
      {/each}
    </div>
    {#if current}
      {@const action = actionFor(current)}
      <div class="ra-caption">
        <span data-testid="discover-recent-meta" class="meta">{caption(current)}</span>
        <button
          data-testid="discover-recent-action"
          class="action"
          disabled={action === 'Installing…' || action === 'Queued'}
          onclick={() => onAction(current)}
        >
          {action}
        </button>
      </div>
    {/if}
  </div>

  <div class="ra-side">
    <div class="ra-list" bind:this={listEl}>
      {#if frame}
        <div
          class="ra-hl"
          aria-hidden="true"
          style:transform={`translateY(${frame.top}px)`}
          style:height={`${frame.height}px`}
        >
          {#key cycle}
            <span data-testid="discover-recent-bar" class="ra-bar" class:paused></span>
          {/key}
        </div>
      {/if}
      {#each games as game, i (game.id)}
        <button
          data-testid={`discover-recent-row-${game.id}`}
          data-active={i === index ? 'true' : 'false'}
          class="ra-row"
          class:active={i === index}
          aria-current={i === index ? 'true' : undefined}
          bind:this={rowEls[i]}
          onclick={() => onRow(i)}
        >
          <span class="ra-cover">
            <Image url={game.path_cover_small || game.path_cover_large} alt="" placeholder="" class="ra-cover-img" />
          </span>
          <span class="ra-text">
            <b>{game.name}</b>
            <span>{labelFor(game)}</span>
          </span>
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .ra {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 250px;
    gap: 16px;
  }

  .ra-main {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .ra-shot {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--r-row);
    overflow: hidden;
    box-shadow: var(--shadow-card);
    background: var(--field);
    cursor: pointer;
    transition:
      transform 150ms ease,
      box-shadow 150ms ease;
  }

  .ra-shot:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-lift);
  }

  .ra-shot:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  .ra-layer {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity var(--m-fade, 600ms) ease;
  }

  .ra-layer.on {
    opacity: 1;
  }

  .ra-caption {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .meta {
    color: var(--text-muted);
    font-size: 12px;
    min-width: 0;
  }

  .action {
    flex: none;
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

  .ra-side {
    position: relative;
    min-height: 200px;
  }

  .ra-list {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 2px;
    scrollbar-width: thin;
  }

  .ra-row {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex: none;
  }

  .ra-row:hover:not(.active) {
    background: var(--surface);
  }

  .ra-cover {
    position: relative;
    flex: none;
    width: 46px;
    aspect-ratio: 3 / 4;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow-card);
  }

  .ra-cover :global(.ra-cover-img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .ra-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .ra-text b {
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ra-text span {
    color: var(--text-muted);
    font-size: 11px;
  }

  .ra-hl {
    position: absolute;
    left: 2px;
    right: 2px;
    top: 0;
    z-index: 0;
    border-radius: var(--r-row);
    background: var(--selected);
    box-shadow: inset 0 0 0 2px var(--primary);
    overflow: hidden;
    pointer-events: none;
    transition:
      transform var(--m-glide, 450ms) cubic-bezier(0.2, 0.7, 0.2, 1),
      height var(--m-glide, 450ms) cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  .ra-bar {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    width: 100%;
    background: var(--primary);
    transform: scaleX(0);
    transform-origin: left center;
    animation: ra-fill 10s linear forwards;
  }

  .ra-bar.paused {
    animation-play-state: paused;
  }

  @keyframes ra-fill {
    to {
      transform: scaleX(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ra-hl,
    .ra-layer,
    .ra-shot {
      transition: none;
    }

    .ra-bar {
      animation: none;
    }
  }
</style>
