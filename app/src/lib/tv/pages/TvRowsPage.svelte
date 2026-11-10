<script lang="ts">
  // A rows page (tvmode.js `rowsPage`): the title, a muted line, the rows,
  // then the footer with the summary and the main button, which Start
  // presses from anywhere on the page; while it is disabled Start (or A on
  // it) says why (items 100–101, 480–481, 513). The Backup page and the
  // drive pages are built on it.
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvPageRows from '../ui/TvPageRows.svelte';
  import type { PageMain, PageRow } from './pageRows';

  let {
    title,
    sub = undefined,
    rows,
    main = null,
    testId = undefined,
  }: { title: string; sub?: string; rows: readonly PageRow[]; main?: PageMain | null; testId?: string } = $props();

  useScreen({
    hints: () => (main && !main.disabled ? [{ button: 'MENU', label: main.hint }] : []),
    main: () => (main ? { disabled: main.disabled, why: main.why, press: main.press } : null),
  });
</script>

<div class="page" data-scroll="page" data-testid={testId}>
  <div class="page-in" data-nav="v" id="rows-page">
    <h1 data-testid="tv-rows-title">{title}</h1>
    {#if sub}<p class="sub" data-testid="tv-rows-sub">{sub}</p>{/if}
    <TvPageRows {rows} />
    {#if main}
      <div class="bk-foot">
        <span class="muted" data-testid="tv-rows-summary">{main.summary}</span>
        <TvButton k="main" primary glyph="MENU" disabled={main.disabled} why={main.why} testId="tv-rows-main" onA={main.press}>{main.label}</TvButton>
      </div>
    {/if}
  </div>
</div>
