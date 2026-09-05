<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { BACKGROUND_CYCLE_MS, CROSS_FADE_MS, cycleIndex, shouldCycle } from './background';
  import { clearIfBottom, initialSlotState, outgoingSlot, withNextCover } from './backgroundSlots';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 360ms). A new image is written only into the slot about to become
  // visible; the outgoing slot's image is left in place until the fade has
  // had time to finish, so both images sit on screen together while the
  // opacity transitions — see `backgroundSlots.ts` for the sequencing.
  let slots = $state(initialSlotState);

  // Resolved URL -> local variant path, for the process lifetime. The path is
  // stable (the cache is content-addressed by URL) and the answer costs an
  // IPC round trip, so re-hovering a card the user has already dwelled on
  // must not pay for it again. Module scoped, like `lastViewed` itself, so it
  // survives a Shell remount.
  const variantPaths = new Map<string, string>();

  // Which of the subject's images is showing. Reset whenever the subject
  // changes, so a new game always starts at its first image.
  let index = $state(0);
  // Read once into a `$derived`: `lastViewed.urls` is a getter that builds a
  // new array on every read, so reading it directly in an effect or a timer
  // callback would churn arrays and never compare equal.
  let urls = $derived(lastViewed.urls);
  $effect(() => {
    // Referencing `urls` is what subscribes this effect to a subject change.
    urls;
    index = 0;
  });

  let current = $derived(urls[index % Math.max(urls.length, 1)] ?? null);

  // 0–60 in the config, 0–0.6 as an opacity.
  let opacity = $derived(uiSettings.backgroundFade / 100);

  // The 5s rotation (fanart_background.py:52-53, 80-84). Only with more than
  // one image, and only while the art is visible — user ruling 2026-09-05.
  $effect(() => {
    if (!shouldCycle(urls, uiSettings.backgroundFade)) return;
    const count = urls.length;
    const timer = setInterval(() => {
      index = cycleIndex(index, count);
    }, BACKGROUND_CYCLE_MS);
    return () => clearInterval(timer);
  });

  $effect(() => {
    const url = current;
    if (url === null) return;
    let cancelled = false;
    // The timeout handle is captured now and cleared on teardown: before
    // this, a rapid sequence of subjects left one pending `clearIfBottom` per
    // change, each firing after the component may already have gone.
    let clearTimer: ReturnType<typeof setTimeout> | null = null;

    function show(path: string) {
      const src = convertFileSrc(path);
      if (slots[slots.top] === src) return; // already showing this image
      slots = withNextCover(slots, src);
      const toClear = outgoingSlot(slots);
      clearTimer = setTimeout(() => {
        clearTimer = null;
        slots = clearIfBottom(slots, toClear);
      }, CROSS_FADE_MS);
    }

    const memoised = variantPaths.get(url);
    if (memoised !== undefined) {
      show(memoised);
    } else {
      api
        .ensureBackgroundVariant(url)
        .then((path) => {
          variantPaths.set(url, path);
          if (!cancelled) show(path);
        })
        .catch(() => {
          // Offline, missing, or a format this build cannot decode. User
          // ruling 2026-09-05: no raw-image fallback — the CSS blur is gone,
          // so the raw source would be a different effect, not a degraded
          // one. Keep whatever is already showing.
        });
    }

    return () => {
      cancelled = true;
      if (clearTimer !== null) clearTimeout(clearTimer);
    };
  });
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
    /* `inset: 0`, not the old `-60px` overscan: the image arrives already
       blurred (`ensure_background_variant`), so nothing samples past the
       element's edges any more and there is nothing to hide. */
    inset: 0;
    background-position: center;
    background-size: cover;
    /* No CSS blur filter here. The blur is baked into the 960px JPEG the
       backend builds once, so the compositor uploads one small texture
       instead of re-blurring ~2.4 Mpx per layer per frame for the whole
       fade. */
    opacity: 0;
    transition: opacity var(--m-slow) ease;
    /* Promotes each layer to its own compositor layer for the fade — the
       only property that animates here. */
    will-change: opacity;
  }

  .layer.visible {
    opacity: var(--art-opacity);
  }
</style>
