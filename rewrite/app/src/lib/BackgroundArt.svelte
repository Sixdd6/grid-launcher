<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 360ms). `front` is the visible one; a new cover loads into the other
  // layer and the two swap once its file path has resolved.
  let front = $state<string | null>(null);
  let back = $state<string | null>(null);
  let frontIsA = $state(true);

  $effect(() => {
    const url = lastViewed.coverUrl;
    if (url === null) return;
    let cancelled = false;
    api
      .ensureImage(url)
      .then((path) => {
        if (cancelled) return;
        const src = convertFileSrc(path);
        if (src === front) return;
        back = src;
        front = src;
        frontIsA = !frontIsA;
      })
      .catch(() => {
        // Offline or missing: keep whatever is already showing.
      });
    return () => {
      cancelled = true;
    };
  });

  // 0–60 in the config, 0–0.6 as an opacity.
  let opacity = $derived(uiSettings.backgroundFade / 100);
</script>

<div data-testid="background-art" class="art" aria-hidden="true" style={`--art-opacity: ${opacity}`}>
  <div class="layer" class:visible={frontIsA} style={frontIsA && front ? `background-image: url("${front}")` : ''}></div>
  <div class="layer" class:visible={!frontIsA} style={!frontIsA && back ? `background-image: url("${back}")` : ''}></div>
</div>

<style>
  .art {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }

  .layer {
    position: absolute;
    /* Overscan: a 40px blur samples past the element's own edges and would
       otherwise fade to the page background at every side. */
    inset: -60px;
    background-position: center;
    background-size: cover;
    filter: blur(40px);
    opacity: 0;
    transition: opacity var(--m-slow) ease;
  }

  .layer.visible {
    opacity: var(--art-opacity);
  }
</style>
