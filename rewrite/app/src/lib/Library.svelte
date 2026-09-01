<script lang="ts">
  import { api, type GameSummary, type Platform } from './api';
  import Cover from './Cover.svelte';
  import Details from './Details.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { isInstalled, refresh as refreshInstalled } from './stores/installed.svelte';

  const COLUMNS = 6;
  let platforms = $state<Platform[]>([]);
  let games = $state<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  let focusIndex = $state(0);
  let gridEl = $state<HTMLElement | null>(null);
  let detailsGame = $state<GameSummary | null>(null);

  let libraryPathInput = $state('');
  let showLibraryBanner = $state(false);
  let libraryPathSaving = $state(false);
  let libraryPathError = $state<string | null>(null);

  let activePlatformName = $derived(platforms.find((p) => p.id === activePlatform)?.name ?? '');

  $effect(() => {
    api.listPlatforms().then((p) => {
      platforms = p;
      if (p.length && activePlatform === null) selectPlatform(p[0].id);
    });
    refreshInstalled();
    checkLibraryPath();
  });

  async function checkLibraryPath() {
    try {
      const path = await api.getLibraryPath();
      showLibraryBanner = path.trim() === '';
    } catch {
      // Unreadable config: leave the banner as-is rather than nag with a
      // guess — install errors will surface the real problem if there is one.
    }
  }

  async function selectPlatform(id: number) {
    activePlatform = id;
    const g = await api.listGames(id);
    if (activePlatform !== id) return; // superseded by a newer selection
    games = g;
    focusIndex = 0;
  }

  function openDetails(game: GameSummary) {
    detailsGame = game;
  }

  function closeDetails() {
    detailsGame = null;
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (detailsGame) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!detailsGame) {
        const game = games[focusIndex];
        if (game) openDetails(game);
      }
      return;
    }
    if (detailsGame) return; // grid navigation is suspended while the overlay is open
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

  async function saveLibraryPath() {
    libraryPathError = null;
    libraryPathSaving = true;
    try {
      await api.setLibraryPath(libraryPathInput.trim());
      showLibraryBanner = false;
    } catch (err) {
      libraryPathError = err instanceof Error ? err.message : String(err);
    } finally {
      libraryPathSaving = false;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if showLibraryBanner}
  <div data-testid="library-path-banner" class="library-banner">
    <span>Set a library folder to install games.</span>
    <input
      data-testid="library-path-input"
      bind:value={libraryPathInput}
      placeholder="/path/to/library"
      disabled={libraryPathSaving}
    />
    <button data-testid="library-path-save" disabled={libraryPathSaving || !libraryPathInput.trim()} onclick={saveLibraryPath}>
      {libraryPathSaving ? 'Saving…' : 'Save'}
    </button>
    {#if libraryPathError}<span class="banner-error" role="alert">{libraryPathError}</span>{/if}
  </div>
{/if}

<nav class="platforms">
  {#each platforms as p (p.id)}
    <button data-testid={`platform-btn-${p.id}`} class:active={p.id === activePlatform} onclick={() => selectPlatform(p.id)}>{p.name}</button>
  {/each}
</nav>

<div class="grid" bind:this={gridEl} style="--columns: {COLUMNS}">
  {#each games as game, i (game.id)}
    <div data-testid={`game-card-${game.id}`} class="card" class:focused={i === focusIndex} onclick={() => openDetails(game)} role="presentation">
      <Cover {game} />
      {#if isInstalled(game, activePlatformName)}
        <span data-testid={`installed-badge-${game.id}`} class="badge">
          <span class="dot"></span>
          Installed
        </span>
      {/if}
    </div>
  {/each}
</div>

{#if detailsGame}
  {#key detailsGame.id}
    <Details
      game={detailsGame}
      platformName={activePlatformName}
      onClose={closeDetails}
      onLibraryPathUnset={() => { showLibraryBanner = true; }}
    />
  {/key}
{/if}

<style>
  .library-banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 10px 24px;
    background: var(--border);
    color: var(--text-h);
    font-size: 13px;
  }

  .library-banner input {
    flex: 1 1 240px;
    min-width: 160px;
    font: inherit;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .library-banner button {
    font: inherit;
    padding: 6px 14px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .library-banner button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .banner-error {
    color: #e5484d;
    flex-basis: 100%;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(var(--columns), 1fr);
    gap: 16px;
    /* Extra bottom padding keeps the last row clear of the fixed downloads footer. */
    padding: 24px 24px 60px;
  }
  .card {
    position: relative;
    aspect-ratio: 3 / 4;
    /* Off-screen cards skip layout/paint; the intrinsic size keeps the
       scrollbar stable at the 3:4 cover ratio. */
    content-visibility: auto;
    contain-intrinsic-size: auto 200px 267px;
    border-radius: 8px;
    overflow: hidden;
    transform: scale(1);
    transition: transform 160ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
    cursor: pointer;
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

  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 11px;
    line-height: 1.4;
    backdrop-filter: blur(2px);
  }

  .badge .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4ade80;
    flex: none;
  }
</style>
