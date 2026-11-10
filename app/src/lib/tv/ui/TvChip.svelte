<script lang="ts">
  // A chip: a pill with a hairline border, optionally with a prompt in front
  // ("(RT) Select"), in the warning style, or focusable (a Server header's
  // emulator and firmware chips open the system's page).
  import type { Snippet } from 'svelte';
  import { tvf } from '../focus/actions';
  import Glyph from '../glyphs/Glyph.svelte';
  import type { GlyphButton } from '../glyphs/glyphs';

  let {
    glyph = undefined,
    warn = false,
    k = undefined,
    al = undefined,
    onA = undefined,
    children,
  }: {
    glyph?: GlyphButton;
    warn?: boolean;
    /** Set with `onA` to make the chip focusable. */
    k?: string;
    al?: string;
    onA?: () => void;
    children: Snippet;
  } = $props();
</script>

{#if onA && k}
  <button class="chip" class:warn data-k={k} data-al={al} use:tvf={{ a: onA }}>{#if glyph}<Glyph button={glyph} />{/if}{@render children()}</button>
{:else}
  <span class="chip" class:warn>{#if glyph}<Glyph button={glyph} />{/if}{@render children()}</span>
{/if}
