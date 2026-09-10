<script lang="ts">
  import { untrack } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { BACKGROUND_CYCLE_MS, backgroundUrls, CROSS_FADE_MS, cycleIndex, shouldCycle } from './background';
  import { rememberVariant, variantKey, variantPaths } from './backgroundPrefetch';
  import { clearIfBottom, initialSlotState, outgoingSlot, withNextCover } from './backgroundSlots';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 220ms, the `--m-base` token below and `CROSS_FADE_MS`, which must agree).
  // A new image is written only into the slot about to become
  // visible; the outgoing slot's image is left in place until the fade has
  // had time to finish, so both images sit on screen together while the
  // opacity transitions — see `backgroundSlots.ts` for the sequencing.
  let slots = $state(initialSlotState);

  // Which of the subject's images is showing. Reset whenever the subject
  // changes, so a new game always starts at its first image.
  let index = $state(0);
  // URLs whose blurred variant the backend could not build. Always REPLACED,
  // never mutated in place, so the `$derived` below sees the change: a failed
  // fanart drops out of its tier, and once the tier is empty the art falls
  // through to the screenshots and then to the cover.
  let failed = $state(new Set<string>());

  // Read once into a `$derived`: `backgroundUrls` builds a new array on every
  // call, so calling it directly in an effect or a timer callback would churn
  // arrays and never compare equal.
  let urls = $derived(backgroundUrls(lastViewed.subject, failed));

  // Declared BEFORE the fetch effect on purpose: effects flush in creation
  // order, so `index` is back at 0 before the fetch effect below reads
  // `current` for the new subject. Swapped, the first frame of a new game
  // would be whatever image the previous game's cycle had reached.
  $effect(() => {
    // Depends on the SUBJECT, not on `urls`: `urls` also changes when a fetch
    // fails, and resetting the index there would restart the cycle at the
    // first image every time one URL fell through. `noteViewed` gates a
    // re-report of the same art, so this fires when the subject's art
    // actually changes, not on every list refresh.
    const subject = lastViewed.subject;
    void subject;
    index = 0;
    // A new game starts with a clean slate — a URL that failed for the
    // previous subject says nothing about this one. Guarded and untracked so
    // this effect neither depends on `failed` nor re-derives `urls` in the
    // usual case, where nothing has failed.
    untrack(() => {
      if (failed.size > 0) failed = new Set();
    });
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
    // Background art off (design §10): nothing is visible, so nothing is
    // worth downloading, decoding and blurring. Turning it back on re-runs
    // this effect, which then fetches.
    if (uiSettings.backgroundFade === 0) return;
    const url = current;
    if (url === null) return;
    // Part of the variant's file name, so a change of sigma re-runs this
    // effect and asks the backend for a different file.
    const blur = uiSettings.backgroundBlur;
    let cancelled = false;
    // The timeout handle is captured now and cleared on teardown: before
    // this, a rapid sequence of subjects left one pending `clearIfBottom` per
    // change, each firing after the component may already have gone.
    let clearTimer: ReturnType<typeof setTimeout> | null = null;

    // `show` both reads and writes `slots`, so every call goes through
    // `untrack`: otherwise a memo hit — which calls it synchronously, inside
    // the effect — would make `slots` a dependency, and the write would
    // immediately re-run the effect whose teardown cancels the
    // `clearIfBottom` timer it had just armed. The outgoing layer's image
    // would then never be dropped.
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

    const key = variantKey(blur, url);
    const memoised = variantPaths.get(key);
    if (memoised !== undefined) {
      untrack(() => show(memoised));
    } else {
      api
        .ensureBackgroundVariant(url, blur)
        .then((path) => {
          rememberVariant(key, path);
          if (!cancelled) untrack(() => show(path));
        })
        .catch(() => {
          // Offline, missing, or a format this build cannot decode. User
          // ruling 2026-09-05: no raw-image fallback — the CSS blur is gone,
          // so the raw source would be a different effect, not a degraded
          // one. Record the URL instead: `urls` re-derives without it, which
          // moves on to the next image or, once the tier empties, down to
          // the screenshots and then the cover. Recorded even when the
          // effect was cancelled — the URL is no better for the next
          // subject that lists it.
          failed = new Set([...failed, url]);
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
    transition: opacity var(--m-base) ease;
    /* Promotes each layer to its own compositor layer for the fade — the
       only property that animates here. */
    will-change: opacity;
  }

  .layer.visible {
    opacity: var(--art-opacity);
  }
</style>
