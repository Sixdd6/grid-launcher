<script lang="ts">
  // The Library rail's RomM group (TVRommStorefront README § Library › RomM
  // group; prototype `rRailGroup`): a folder like a store's, with RomM's mark
  // and name (A opens and closes it), its flag (a spinner while connecting,
  // `!` for Can't reach and Sign-in expired, `off` for not connected), then
  // Installed (count) and one row per platform with RomM's game
  // count (`romm/rail.ts` `rommRailGroup`). Its own panel; open or closed is
  // the list desktop and TV mode share (`[ui] rail_closed`).
  import type { RommRailGroup, RommRailKey } from '../../../../romm/rail';
  import TvRailRow from '../../../ui/TvRailRow.svelte';
  import TvRommMark from '../../../romm/TvRommMark.svelte';
  import { ROMM_FOLD_KEY, rommRowKey } from '../../../romm/tvRomm.svelte';
  import { toggleRailGroup, uiSettings } from '../../../../stores/uiSettings.svelte';
  import { railClosedCount } from '../../../../library/railFold';

  let { group, selected, onSelect }: { group: RommRailGroup; selected: RommRailKey | null; onSelect: (key: RommRailKey) => void } = $props();

  const tid = (testId: string) => testId.replace(/^library-rail-/, 'tv-lib-rail-');
  let open = $derived(!uiSettings.railGroupClosed('romm'));
  let count = $derived(railClosedCount('romm', group.rows.find((r) => r.kind === 'installed')?.count ?? null));
</script>

<div class="rgrp" class:closed={!open} data-grp="romm">
<TvRailRow
  k={ROMM_FOLD_KEY}
  fold={open}
  holdsSel={!open && selected !== null}
  closedCount={count}
  testId="tv-lib-fold-romm"
  onToggle={() => toggleRailGroup('romm')}
  ><TvRommMark />RomM{#if group.flag}{#if group.flag.text === 'busy'}<span class="sflag rbusy" aria-label={group.flag.title} data-testid="tv-lib-romm-flag"><span class="sspin"></span></span>{:else}<span
        class="sflag"
        class:off={group.flag.text === 'off'}
        aria-label={group.flag.title}
        data-testid="tv-lib-romm-flag">{group.flag.text}</span
      >{/if}{/if}</TvRailRow
>
{#if open}
  {#each group.rows as r (r.key)}
    <TvRailRow k={rommRowKey(r.key)} sub selected={selected === r.key} count={r.count} testId={tid(r.testId)} onSelect={() => onSelect(r.key)}>{r.label}</TvRailRow>
  {/each}
{/if}
</div>
