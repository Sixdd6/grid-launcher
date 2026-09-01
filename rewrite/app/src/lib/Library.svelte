<script lang="ts">
  import { api, type GameSummary, type Platform } from './api';
  import Cover from './Cover.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';

  const COLUMNS = 6;
  let platforms = $state<Platform[]>([]);
  let games = $state<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  let focusIndex = $state(0);
  let gridEl = $state<HTMLElement | null>(null);

  $effect(() => {
    api.listPlatforms().then((p) => {
      platforms = p;
      if (p.length && activePlatform === null) selectPlatform(p[0].id);
    });
  });

  async function selectPlatform(id: number) {
    activePlatform = id;
    focusIndex = 0;
    games = await api.listGames(id);
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'accept' || action === 'back') return; // skeleton: navigation only
    focusIndex = moveFocus(focusIndex, action, COLUMNS, games.length);
    gridEl?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      handleNav(action);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<nav class="platforms">
  {#each platforms as p (p.id)}
    <button class:active={p.id === activePlatform} onclick={() => selectPlatform(p.id)}>{p.name}</button>
  {/each}
</nav>

<div class="grid" bind:this={gridEl} style="--columns: {COLUMNS}">
  {#each games as game, i (game.id)}
    <div class="card" class:focused={i === focusIndex}>
      <Cover {game} />
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--columns), 1fr);
    gap: 16px;
    padding: 24px;
    content-visibility: auto;
  }
  .card {
    aspect-ratio: 3 / 4;
    border-radius: 8px;
    overflow: hidden;
    transform: scale(1);
    transition: transform 160ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
  }
  .card.focused {
    transform: scale(1.08);
    outline: 3px solid #7aa2ff;
    z-index: 1;
  }
  .card :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
