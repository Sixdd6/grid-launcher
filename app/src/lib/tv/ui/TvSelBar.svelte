<script lang="ts">
  // The bar pinned under a grid in select mode (item 445; tvmode.js
  // `selPaint`, tvmode.css `.selbar`): "<n> selected · <size>", then X
  // "Select all <n>" (or "All <n> selected · Clear"), Y "Backup to drive…"
  // (on a drive "Install or remove") and RT "Done". Each hint can be clicked
  // like the hint bar's (`data-btn`).
  import Glyph from '../glyphs/Glyph.svelte';
  import { select, selBar, type SelGame } from '../select.svelte';

  let { games, drive = false }: { games: readonly SelGame[]; drive?: boolean } = $props();

  let bar = $derived(selBar(select.ids, games, drive));
</script>

{#if select.on}
  <div class="selbar" data-testid="tv-selbar">
    <b data-testid="tv-selbar-summary">{bar.summary}</b>
    <span class="sp"></span>
    <span class="hint" data-btn="X" data-testid="tv-selbar-x"><Glyph button="X" />{bar.x}</span>
    <span class="hint" data-btn="Y" data-testid="tv-selbar-y"><Glyph button="Y" />{bar.y}</span>
    <span class="hint" data-btn="RT" data-testid="tv-selbar-rt"><Glyph button="RT" />{bar.rt}</span>
  </div>
{/if}
