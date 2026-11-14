<script lang="ts">
  // Settings › Stores (TVStores README § Settings › Stores; prototype
  // `setPaint` for `stores`): a row per store with its mark, "Signed in as …"
  // (or "Not signed in") and its status (Connected, Login expired, Not
  // connected). A opens the store's page. One row per enabled store (GOG, Epic Games).
  import { ENABLED_STORES, storeName } from '../../gamestores/marks';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { tvf } from '../focus/actions';
  import TvIcon from '../ui/TvIcon.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { openStorePage } from './storeActions';
  import { STORES_PANE_NOTE, storeRowPill, storeRowSub } from './storeWords';
</script>

<div class="pane-h"><h1 data-testid="tv-pane-title">Stores</h1></div>
<p class="note" style="margin-left:1.2rem">{STORES_PANE_NOTE}</p>
{#each ENABLED_STORES as source (source)}
  {@const st = gameStores.status(source)}
  {#if st}
    {@const pill = storeRowPill(st)}
    <div class="srow" data-k={`sto-${source}`} data-al="Open" data-testid={`tv-stores-row-${source}`} use:tvf={{ a: () => openStorePage(source) }}>
      <TvStoreMark {source} lg />
      <div class="lab"><div class="t">{storeName(source)}</div><div class="s">{storeRowSub(st)}</div></div>
      <div class="val"><span class="st" class:ok={pill.tone === 'ok'} class:warn={pill.tone === 'warn'}>{pill.text}</span><span class="chev"><TvIcon name="chevronRight" px={18} /></span></div>
    </div>
  {:else}
    <p class="note" data-testid="tv-stores-loading"><span class="sspin"></span> Loading…</p>
  {/if}
{/each}
