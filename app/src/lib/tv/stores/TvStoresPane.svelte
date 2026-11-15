<script lang="ts">
  // Settings › Game stores (TVStores README § Settings › Stores;
  // TVRommStorefront README § Settings › Game stores): one row per
  // storefront in name order, RomM between GOG and Steam, each with its mark,
  // its line ("Signed in as …", "Not signed in"; RomM "Signed in as sam ·
  // romm.home.lan", "Not signed in · …", "Your own game server") and its
  // status pill (RomM: Connected, Can't reach, Sign-in expired, Not
  // connected, or a spinner with "Connecting…"). A opens the store's page,
  // or the RomM page.
  import { ENABLED_STORES, storeName } from '../../gamestores/marks';
  import { inStorefrontOrder } from '../../romm/order';
  import { rommAccountLine, rommPill } from '../../romm/words';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { romm } from '../../stores/romm.svelte';
  import { tvf } from '../focus/actions';
  import { openRommPage } from '../romm/rommActions';
  import { GAME_STORES_NOTE, GAME_STORES_TITLE } from '../romm/rommTv';
  import TvRommMark from '../romm/TvRommMark.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { openStorePage } from './storeActions';
  import { storeRowPill, storeRowSub } from './storeWords';

  const rows = inStorefrontOrder<string>([...ENABLED_STORES, 'romm'], (s) => s);
  let pill = $derived(rommPill(romm.status));
</script>

<div class="pane-h"><h1 data-testid="tv-pane-title">{GAME_STORES_TITLE}</h1></div>
<p class="note" style="margin-left:1.2rem">{GAME_STORES_NOTE}</p>
{#each rows as source (source)}
  {#if source === 'romm'}
    <div class="srow" data-k="sto-romm" data-al="Open" data-testid="tv-stores-row-romm" use:tvf={{ a: openRommPage }}>
      <span style="margin-right:.2rem"><TvRommMark lg /></span>
      <div class="lab"><div class="t">RomM</div><div class="s" data-testid="tv-stores-row-romm-sub">{rommAccountLine(romm.status)}</div></div>
      <div class="val">
        <span class="st" class:ok={pill.tone === 'ok'} class:warn={pill.tone === 'warn'} data-testid="tv-stores-row-romm-pill"
          >{#if pill.tone === 'busy'}<span class="sspin"></span> {/if}{pill.text}</span
        ><span class="chev"><TvIcon name="chevronRight" px={18} /></span>
      </div>
    </div>
  {:else}
    {@const store = source as (typeof ENABLED_STORES)[number]}
    {@const st = gameStores.status(store)}
    {#if st}
      {@const p = storeRowPill(st)}
      <div class="srow" data-k={`sto-${store}`} data-al="Open" data-testid={`tv-stores-row-${store}`} use:tvf={{ a: () => openStorePage(store) }}>
        <TvStoreMark source={store} lg />
        <div class="lab"><div class="t">{storeName(store)}</div><div class="s">{storeRowSub(st)}</div></div>
        <div class="val"><span class="st" class:ok={p.tone === 'ok'} class:warn={p.tone === 'warn'}>{p.text}</span><span class="chev"><TvIcon name="chevronRight" px={18} /></span></div>
      </div>
    {:else}
      <p class="note" data-testid="tv-stores-loading"><span class="sspin"></span> Loading…</p>
    {/if}
  {/if}
{/each}
