<script lang="ts">
  // The selection bar of select mode (GameBackups README; prototype
  // `bkSelBar`): pinned to the bottom of the grid panel, with the count and
  // total size, Select all N / All N selected, Clear and the view's action
  // ("Backup to drive…" on Server; Library's drive view passes its own).
  import type { Snippet } from 'svelte';
  import { selectAllLabel, selectionSummary } from './selection';

  let {
    testId = 'selection-bar',
    count,
    size,
    visibleCount,
    allSelected,
    onSelectAll,
    onClear,
    actions,
  }: {
    testId?: string;
    count: number;
    /** The picked games' total size, formatted. */
    size: string;
    visibleCount: number;
    allSelected: boolean;
    onSelectAll: () => void;
    onClear: () => void;
    /** The bar's main buttons, on the right. */
    actions: Snippet;
  } = $props();

  let summary = $derived(selectionSummary(count, size));
</script>

<div data-testid={testId} class="bar" role="toolbar" aria-label="Selected games">
  <span class="count" aria-live="polite">
    <b data-testid={`${testId}-count`}>{summary.title}</b>
    <span class="note">{summary.note}</span>
  </span>
  <button data-testid={`${testId}-all`} class="g-btn secondary small" disabled={visibleCount === 0} onclick={onSelectAll}>
    {selectAllLabel(visibleCount, allSelected)}
  </button>
  {#if count > 0}
    <button data-testid={`${testId}-clear`} class="g-btn secondary small" onclick={onClear}>Clear</button>
  {/if}
  <span class="gap"></span>
  {@render actions()}
</div>

<style>
  .bar {
    flex: none;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: var(--gutter);
    padding: 10px 14px;
    border-radius: var(--r-panel);
    border: 1px solid var(--line);
    background: var(--overlay);
    box-shadow: var(--shadow-overlay);
  }

  .count {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .count b {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-h);
  }

  .note {
    font-size: 12px;
    color: var(--text-muted);
  }

  .small {
    font-size: 12px;
    padding: 5px 12px;
  }

  .gap {
    flex: 1 1 auto;
  }
</style>
