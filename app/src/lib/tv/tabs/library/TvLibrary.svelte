<script lang="ts">
  // Library (P5 placeholder: the rail and an empty grid). P8 builds the
  // filters, folders, counts, header and covers (items 176–195).
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
  import TvGrid from '../../ui/TvGrid.svelte';

  const ROWS = [
    ['all', 'All games'],
    ['recent', 'Recent'],
    ['updates', 'Updates'],
  ] as const;
  let rail = $state<string>('all');
  let name = $derived(ROWS.find(([k]) => k === rail)?.[1] ?? '');
</script>

<div class="split" data-testid="tv-library">
  <TvRail label="Library">
    {#each ROWS as [key, label] (key)}
      <TvRailRow k={`r-${key}`} selected={rail === key} testId={`tv-lib-rail-${key}`} onSelect={() => (rail = key)}>{label}</TvRailRow>
    {/each}
  </TvRail>
  <div class="main">
    <div class="vh"><h1>{name}</h1></div>
    <TvGrid id="lib-grid" scroll="lib-grid" />
  </div>
</div>
