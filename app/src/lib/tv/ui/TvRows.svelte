<script lang="ts">
  // Settings-style rows (rows.ts; items 21, 29, 129): switches (A flips),
  // choices (A opens a list), sliders (Left / Right adjust while focused),
  // text (A opens the keyboard), actions, and info rows that aren't
  // focusable. A focused row takes the `selected` fill. Put it inside an
  // area with `data-nav="v"` (a pane, a page).
  import { tvf } from '../focus/actions';
  import { choiceLabel, rowA, sliderPercent, sliderStep, textValue, type Row } from '../rows';
  import TvIcon from './TvIcon.svelte';
  import TvSlider from './TvSlider.svelte';
  import TvSwitch from './TvSwitch.svelte';

  let { rows }: { rows: readonly Row[] } = $props();

  function handlers(r: Row) {
    switch (r.type) {
      case 'switch':
        return { a: () => r.set(!r.on) };
      case 'choice':
        return { a: () => r.choose() };
      case 'slider':
        return { adjust: (d: -1 | 1) => r.set(sliderStep(r, d)) };
      case 'text':
        return { a: () => r.edit() };
      case 'action':
        return { a: () => r.run() };
      default:
        return {};
    }
  }
</script>

{#each rows as r (r.key)}
  {#if r.type === 'info'}
    <div class="srow info" data-testid={`tv-row-${r.key}`}>
      <div class="lab"><div class="t">{r.label}</div>{#if r.sub}<div class="s">{r.sub}</div>{/if}</div>
      <div class="val">{r.value}</div>
    </div>
  {:else}
    <div
      class="srow"
      class:act={r.type === 'action'}
      class:dng={r.danger}
      data-k={`s-${r.key}`}
      data-al={rowA(r)}
      data-adj={r.type === 'slider' ? '' : undefined}
      data-why={r.disabled ? r.why : undefined}
      aria-disabled={r.disabled ? 'true' : undefined}
      data-testid={`tv-row-${r.key}`}
      use:tvf={handlers(r)}
    >
      <div class="lab"><div class="t">{r.label}</div>{#if r.sub}<div class="s">{r.sub}</div>{/if}</div>
      <div class="val">
        {#if r.type === 'switch'}
          <TvSwitch on={r.on} />
        {:else if r.type === 'choice'}
          {choiceLabel(r)} <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
        {:else if r.type === 'slider'}
          <TvSlider percent={sliderPercent(r)} text={r.format(r.value)} />
        {:else if r.type === 'text'}
          {@const tv = textValue(r)}
          {#if tv.unset}<i>{tv.text}</i>{:else}{tv.text}{/if} <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
        {:else}
          {#if r.value}{r.value} {/if}<span class="chev"><TvIcon name="chevronRight" px={18} /></span>
        {/if}
      </div>
    </div>
  {/if}
{/each}
