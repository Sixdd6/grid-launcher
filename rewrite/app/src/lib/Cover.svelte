<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api, type GameSummary } from './api';

  let { game }: { game: GameSummary } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    let cancelled = false;
    src = null;
    if (game.path_cover_small) {
      api.ensureCover(game.id, game.path_cover_small).then((path) => {
        if (!cancelled) src = convertFileSrc(path);
      }).catch(() => {}); // missing cover: placeholder stays
    }
    return () => { cancelled = true; };
  });
</script>

{#if src}
  <img {src} alt={game.name} loading="lazy" draggable="false" />
{:else}
  <div class="placeholder">{game.name}</div>
{/if}

<style>
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    background: #2a2d34;
    color: #aab;
    font-size: 0.8rem;
    text-align: center;
    padding: 8px;
  }
</style>
