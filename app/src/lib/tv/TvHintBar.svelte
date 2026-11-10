<script lang="ts">
  // The hint bar (items 127–138; tvmode.js `paintHints`): on the left the
  // controller in use, or the offline / no-server notice; on the right what
  // each button does now, in the controller's glyphs. Each hint is a button:
  // clicking it presses that button (item 134; the router handles
  // `data-btn`). It repaints with every focus change, screen change and
  // state change, since everything it reads is reactive.
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

  let left = $derived(
    hintLeft({
      noServer: session.offline && !session.connected,
      offline: !session.offline && !session.connected,
      startup: nav.startup,
      padName: controllers.name,
    }),
  );
</script>

<footer class="hints" data-testid="tv-hints">
  <div class="left" data-testid="tv-hints-left">
    {#if left.kind === 'offline'}
      <span class="offline-pill">{left.pill}</span><span>{left.text}</span>
    {:else}
      <span>{left.text}</span>
    {/if}
  </div>
  {#each hints as h, i (i)}
    <button class="hint" data-btn={h.button} data-testid={`tv-hint-${h.button}`}><Glyph button={h.button} />{h.label}</button>
  {/each}
</footer>
