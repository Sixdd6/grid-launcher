<script lang="ts">
  // The rows of a rows page (pages/pageRows.ts; tvmode.js `rowsPage`,
  // tvmode.css `.radio`, `.ubar`, `.srow.warnrow`, `.srctag`): headings,
  // notes, info rows, and focusable rows that are switches, radios, choices
  // or actions. Put it inside an area with `data-nav="v"` (a page).
  import { tvf } from '../focus/actions';
  import { pageRowA, type Bar, type PageRow } from '../pages/pageRows';
  import TvIcon from './TvIcon.svelte';
  import TvSwitch from './TvSwitch.svelte';

  let { rows }: { rows: readonly PageRow[] } = $props();
</script>

{#snippet bar(b: Bar)}
  <span class="ubar" class:wide={b.wide} role="img" aria-label={b.label}>
    {#each b.segs as s, i (i)}
      {#if s.kind === 'adding'}
        <i class:over={s.over} style:width={`${s.pct.toFixed(2)}%`}></i>
      {:else if s.kind === 'grid'}
        <i class="grid" style:width={`${s.pct.toFixed(2)}%`}></i>
      {:else}
        <b class:res={s.kind === 'reserved'} style:width={`${s.pct.toFixed(2)}%`}></b>
      {/if}
    {/each}
  </span>
  {#if b.after}<span class="bar-after" data-testid="tv-pr-bar-after">{b.after}</span>{/if}
{/snippet}

{#each rows as r (r.key)}
  {#if r.kind === 'heading'}
    <div class="sgroup" data-testid={`tv-pr-${r.key}`}>{r.text}</div>
  {:else if r.kind === 'note'}
    <p class="note" class:warn={r.warn} class:err={r.err} data-testid={`tv-pr-${r.key}`}>{r.text}</p>
  {:else if r.kind === 'info'}
    <div class="srow info" data-testid={`tv-pr-${r.key}`}>
      <div class="lab">
        <div class="t">{r.label}</div>
        {#if r.sub || r.bar}<div class="s">{#if r.bar}{@render bar(r.bar)}{/if}{#if r.sub}{r.sub}{/if}</div>{/if}
      </div>
      <div class="val">
        {#if r.tag}<span class="srctag">{r.tag}</span>{/if}
        {#if r.check}<span class="fixed" aria-label="Always included"><TvIcon name="check" px={18} /></span>{/if}
        {#if r.value !== undefined}{r.value}{/if}
      </div>
    </div>
  {:else}
    <div
      class="srow"
      class:dng={r.danger}
      class:warnrow={r.warn}
      data-k={r.key}
      data-al={pageRowA(r)}
      data-why={r.disabled ? r.why : undefined}
      aria-disabled={r.disabled ? 'true' : undefined}
      data-testid={`tv-pr-${r.key}`}
      use:tvf={{ a: r.run }}
    >
      {#if r.radio !== undefined}<span class="radio" class:on={r.radio} role="radio" aria-checked={r.radio}></span>{/if}
      <div class="lab">
        <div class="t">{r.label}{#if r.muted}<span class="muted">{r.muted}</span>{/if}</div>
        {#if r.sub !== undefined || r.bar}
          <div class="s">
            {#if r.sub !== undefined}<span class:err={r.subErr} data-testid={`tv-pr-${r.key}-sub`}>{r.sub}</span>{/if}
            {#if r.bar}{@render bar(r.bar)}{/if}
          </div>
        {/if}
      </div>
      {#if r.radio === undefined}
        <div class="val">
          {#if r.toggle !== undefined}
            <TvSwitch on={r.toggle} />
          {:else}
            {#if r.value !== undefined}<span class:warning-text={r.valueWarn} data-testid={`tv-pr-${r.key}-val`}>{r.value}</span>{/if}
            <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
{/each}

