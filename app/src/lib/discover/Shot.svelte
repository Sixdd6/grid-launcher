<script lang="ts">
  // One game's picture in Recently added's 16:9 frame. A picture of another
  // shape (8:7, 4:3, 21:9, a portrait cover) is fitted inside, uncropped,
  // over a blurred, darkened, enlarged copy of itself, so the bars read as
  // part of the picture. The logo sits bottom-left over a soft dark fade.
  import type { GameSummary } from '../api';
  import Image from '../Image.svelte';
  import GameLogo from './GameLogo.svelte';
  import { isWide, shotSource } from './feed';

  let { game }: { game: GameSummary } = $props();

  let source = $derived(shotSource(game));
  let size = $state<{ w: number; h: number } | null>(null);
  let wide = $derived(size ? isWide(size.w, size.h) : true);

  $effect(() => {
    void source?.url;
    size = null;
  });
</script>

<div class="shot" data-kind={source?.kind ?? 'none'}>
  {#if source}
    <div class="fill" aria-hidden="true">
      <Image url={source.url} alt="" placeholder="" class="fill-img" />
    </div>
    <div
      class="pic"
      class:wide
      class:tall={!wide}
      style:aspect-ratio={size ? `${size.w} / ${size.h}` : '16 / 9'}
    >
      <Image
        url={source.url}
        alt=""
        placeholder=""
        class="pic-img"
        onmeasure={(w: number, h: number) => (size = { w, h })}
      />
    </div>
  {/if}
  <div class="logo-wrap">
    <GameLogo id={game.id} title={game.name} logoUrl={game.logo_url} shotUrl={source?.url ?? null} />
  </div>
</div>

<style>
  .shot {
    position: absolute;
    inset: 0;
    overflow: hidden;
    container-type: inline-size;
    background: var(--surface-2);
  }

  .fill {
    position: absolute;
    inset: -12%;
    filter: blur(26px) brightness(0.55) saturate(1.15);
    transform: scale(1.1);
  }

  .fill :global(.fill-img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .fill :global(.skeleton),
  .fill :global(.placeholder) {
    display: none;
  }

  .pic {
    position: absolute;
    inset: 0;
    margin: auto;
    overflow: hidden;
    box-shadow: 0 0 24px rgba(0, 0, 0, 0.35);
  }

  .pic.wide {
    width: 100%;
    height: auto;
  }

  .pic.tall {
    height: 100%;
    width: auto;
  }

  .pic :global(.pic-img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .logo-wrap {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 40px 20px 16px;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.6), rgba(0, 0, 0, 0));
    pointer-events: none;
  }
</style>
