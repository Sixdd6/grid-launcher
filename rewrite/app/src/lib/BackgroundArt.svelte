<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { CROSS_FADE_MS } from './background';
  import { clearIfBottom, initialSlotState, outgoingSlot, withNextCover } from './backgroundSlots';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 360ms). A new cover is written only into the slot about to become
  // visible; the outgoing slot's image is left in place until the fade has
  // had time to finish, so both images sit on screen together while the
  // opacity transitions — see `backgroundSlots.ts` for the sequencing.
  let slots = $state(initialSlotState);

  $effect(() => {
    const url = lastViewed.urls[0];
    if (url === undefined) return;
    let cancelled = false;
    api
      .ensureImage(url)
      .then((path) => {
        if (cancelled) return;
        const src = convertFileSrc(path);
        if (slots[slots.top] === src) return; // already showing this cover
        slots = withNextCover(slots, src);
        const toClear = outgoingSlot(slots);
        setTimeout(() => {
          slots = clearIfBottom(slots, toClear);
        }, CROSS_FADE_MS);
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
  <div class="layer" class:visible={slots.top === 'a'} style={slots.a ? `background-image: url("${slots.a}")` : ''}></div>
  <div class="layer" class:visible={slots.top === 'b'} style={slots.b ? `background-image: url("${slots.b}")` : ''}></div>
</div>

<style>
  .art {
    position: fixed;
    inset: 0;
    /* Behind every positioned view wrapper and the unpositioned shell
       chrome (the `session-error` line), which paint at z 0. */
    z-index: -1;
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
