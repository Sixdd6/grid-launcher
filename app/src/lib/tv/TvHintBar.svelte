<script lang="ts">
  // The hint bar (items 127–138; tvmode.js `paintHints`): on the left the
  // controller in use ("Keyboard" while the keyboard leads), or the offline /
  // no-server notice; on the right what each button does now, in the glyphs
  // of what was pressed last. TV mode ignores the mouse, so a hint is only a
  // label (`data-btn` names its button for the tests). It repaints with every
  // focus change, screen change and state change, since everything it reads
  // is reactive.
  import { untrack } from 'svelte';
  import { session } from '../stores/session.svelte';
  import Glyph from './glyphs/Glyph.svelte';
  import { buildHints, hintLeft } from './hints';
  import { controllers } from './input/router.svelte';
  import { focused, nav } from './nav.svelte';
  import { currentSelHints, select } from './select.svelte';

  let hints = $derived.by(() => {
    const h = nav.hooks;
    const f = nav.focus;
    // Picks change the A and X labels.
    void select.ids;
    void select.on;
    const main = h.main?.() ?? null;
    const layer = nav.layers[nav.layers.length - 1] ?? null;
    return buildHints({
      layer: layer ? { hints: typeof layer.hints === 'function' ? layer.hints() : layer.hints, blocking: layer.blocking } : null,
      focus: f.present ? { a: f.a, disabled: f.disabled, x: f.x } : null,
      // Select mode's own hints replace the rest (item 446); the pages
      // pushed over the grid keep theirs.
      select: nav.pages.length === 0 ? currentSelHints(focused(), f.a) : null,
      // `f.key` and `f.a` are read above, so a focus change re-runs this.
      screen: h.hints?.(focused()) ?? [],
      pages: nav.pages.length,
      startup: nav.startup,
      main: main !== null && (!main.disabled || !!main.why),
    });
  });

  // Keyed by button, so a hint that comes or goes (X Install on Discover's
  // covers) leaves the others' nodes alone: keyed by position, every hint
  // after it swapped its glyph mask, and the masks flickered as focus moved.
  let keyed = $derived.by(() => {
    const seen = new Map<string, number>();
    return hints.map((h) => {
      const n = seen.get(h.button) ?? 0;
      seen.set(h.button, n + 1);
      return { ...h, key: `${h.button}#${n}` };
    });
  });

  // Flipped on every change of hints, so the bar runs one of two identical
  // refresh animations (`tv.css`); swapping the animation's name restarts it.
  // On WebKitGTK 2.54's Skia compositor with the GPU on, a change to the bar
  // while the background art crossfaded reached only some of the frame
  // buffers, so the bar flipped between the new hints and an earlier
  // cover's (2026-11-11). Its own layer did not help. The animation makes
  // the compositor redraw the bar every frame for a while, which repaints it
  // into every buffer, as `WEBKIT_SHOW_DAMAGE=1` did.
  let refresh = $state(false);
  $effect(() => {
    void keyed;
    refresh = !untrack(() => refresh);
  });

  let left = $derived(
    hintLeft({
      noServer: session.offline && !session.connected,
      offline: !session.offline && !session.connected,
      startup: nav.startup,
      padName: controllers.hintName,
    }),
  );
</script>

<footer class="hints" class:refresh-a={refresh} class:refresh-b={!refresh} data-testid="tv-hints">
  <div class="left" data-testid="tv-hints-left">
    {#if left.kind === 'offline'}
      <span class="offline-pill">{left.pill}</span><span>{left.text}</span>
    {:else}
      <span>{left.text}</span>
    {/if}
  </div>
  {#each keyed as h (h.key)}
    <button class="hint" class:off={h.off} data-btn={h.button} data-testid={`tv-hint-${h.button}`} aria-disabled={h.off ? 'true' : undefined}><Glyph button={h.button} />{h.label}</button>
  {/each}
</footer>
