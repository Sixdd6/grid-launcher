<script lang="ts">
  // A TV button (items 22, 24, 28): a pill at least 3rem tall, default,
  // primary or danger. Disabled buttons stay focusable at 45% so A on them
  // can say why (`why`, shown as an error notice). `aria-disabled`, not
  // `disabled`, so it stays an item focus can reach. `glyph` puts a prompt in
  // front of the label (a page's main button shows Menu).
  import type { Snippet } from 'svelte';
  import { tvf } from '../focus/actions';
  import Glyph from '../glyphs/Glyph.svelte';
  import type { GlyphButton } from '../glyphs/glyphs';

  let {
    k,
    primary = false,
    danger = false,
    disabled = false,
    why = undefined,
    al = undefined,
    xl = undefined,
    autofocus = false,
    glyph = undefined,
    testId = undefined,
    onA,
    onX = undefined,
    children,
  }: {
    /** Stable identity (`data-k`). */
    k: string;
    primary?: boolean;
    danger?: boolean;
    disabled?: boolean;
    why?: string;
    /** The A hint (default "Select"). */
    al?: string;
    /** The X hint, when X does something here. */
    xl?: string;
    autofocus?: boolean;
    glyph?: GlyphButton;
    testId?: string;
    onA: () => void;
    onX?: () => void;
    children: Snippet;
  } = $props();
</script>

<button
  class="btn"
  class:pri={primary}
  class:dng={danger}
  data-k={k}
  data-al={al}
  data-xl={xl}
  data-why={why}
  data-autofocus={autofocus ? '' : undefined}
  data-testid={testId}
  aria-disabled={disabled ? 'true' : undefined}
  use:tvf={{ a: onA, x: onX }}
>
  {#if glyph && !disabled}<Glyph button={glyph} />{/if}{@render children()}
</button>
