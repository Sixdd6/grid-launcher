<script lang="ts">
  // The Library rail's store folders (TVStores README § Library; prototype
  // `libRailHTML`), under Backup drives: one folder per store with its mark
  // and name (A opens and closes it; open by default), Installed and
  // Available under it with their counts. The folder shows `!` when the
  // store needs attention and `off` when it isn't connected but still has
  // games installed; Available's count is a spinner while the owned list
  // loads and `!` while it can't list games. One folder per store shown
  // (`storeRailGroups`: Epic Games, GOG). The RomM group (`romm`) sits among
  // them by name: Amazon Games, Epic Games, GOG, RomM, Steam
  // (`STOREFRONT_ORDER`, TVRommStorefront README).
  import type { Snippet } from 'svelte';
  import { storefrontGroups } from '../tabs/library/libraryRail';
  import { availBlock, storeRailGroups, type StorePart } from '../../gamestores/rail';
  import { storeName } from '../../gamestores/marks';
  import type { StoreSource, StoreStatus } from '../../api';
  import { gameStores } from '../../stores/gameStores.svelte';
  import TvRailRow from '../ui/TvRailRow.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { storeFoldKey, storeRowKey, toggleStoreFold, tvStores } from './storeState.svelte';

  let { onSelect, romm = undefined }: { onSelect: (source: StoreSource, part: StorePart) => void; romm?: Snippet } = $props();

  let entries = $derived(storefrontGroups(storeRailGroups(gameStores.statuses, (s) => gameStores.library(s)), !!romm));

  /** `!` (login expired, the tool missing, the owned list failed) or `off` (not connected). */
  function flag(s: StoreStatus | null): '!' | 'off' | null {
    if (!s) return null;
    const b = availBlock(s);
    if (s.connection === 'expired' || b === 'tool' || b === 'error') return '!';
    return s.connection === 'not_connected' ? 'off' : null;
  }
</script>

{#each entries as g (g === 'romm' ? 'romm' : g.source)}
  {#if g === 'romm'}
    {@render romm?.()}
  {:else}
  {@const st = gameStores.status(g.source)}
  {@const f = flag(st)}
  {@const open = !tvStores.folded(g.source)}
  <TvRailRow
    k={storeFoldKey(g.source)}
    fold={open}
    holdsSel={!open && tvStores.view?.source === g.source}
    testId={`tv-lib-fold-store-${g.source}`}
    onToggle={() => toggleStoreFold(g.source)}
    ><TvStoreMark source={g.source} />{storeName(g.source)}{#if f}<span class="sflag" class:off={f === 'off'} data-testid={`tv-lib-store-flag-${g.source}`}>{f}</span>{/if}</TvRailRow
  >
  {#if open}
    {#each g.rows as r (r.key)}
      <TvRailRow
        k={storeRowKey(g.source, r.part)}
        sub
        selected={tvStores.viewKey === r.key}
        count={r.count.kind === 'number' ? r.count.n : r.count.kind === 'attention' ? '!' : null}
        countSpin={r.count.kind === 'loading'}
        countWarn={r.count.kind === 'attention'}
        testId={`tv-lib-rail-store-${g.source}-${r.part}`}
        onSelect={() => onSelect(g.source, r.part)}>{r.label}</TvRailRow
      >
    {/each}
  {/if}
  {/if}
{/each}
