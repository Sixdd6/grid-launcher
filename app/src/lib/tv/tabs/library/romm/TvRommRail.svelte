<script lang="ts">
  // The Library rail's RomM group (TVRommStorefront README § Library › RomM
  // group; prototype `rRailGroup`): a folder like a store's, with RomM's mark
  // and name (A opens and closes it), its flag (a spinner while connecting,
  // `!` for Can't reach and Sign-in expired, `off` for not connected), then
  // Installed (count), Discover, and one row per platform with RomM's game
  // count (`romm/rail.ts` `rommRailGroup`).
  import type { RommRailGroup, RommRailKey } from '../../../../romm/rail';
  import TvRailRow from '../../../ui/TvRailRow.svelte';
  import TvRommMark from '../../../romm/TvRommMark.svelte';
  import { ROMM_FOLD_KEY, rommRowKey, toggleRommFold, tvRomm } from '../../../romm/tvRomm.svelte';

  let { group, selected, onSelect }: { group: RommRailGroup; selected: RommRailKey | null; onSelect: (key: RommRailKey) => void } = $props();

  const tid = (testId: string) => testId.replace(/^library-rail-/, 'tv-lib-rail-');
</script>

<TvRailRow k={ROMM_FOLD_KEY} fold={tvRomm.open} holdsSel={!tvRomm.open && selected !== null} testId="tv-lib-fold-romm" onToggle={toggleRommFold}
  ><TvRommMark />RomM{#if group.flag}{#if group.flag.text === 'busy'}<span class="sflag rbusy" aria-label={group.flag.title} data-testid="tv-lib-romm-flag"><span class="sspin"></span></span>{:else}<span
        class="sflag"
        class:off={group.flag.text === 'off'}
        aria-label={group.flag.title}
        data-testid="tv-lib-romm-flag">{group.flag.text}</span
      >{/if}{/if}</TvRailRow
>
{#if tvRomm.open}
  {#each group.rows as r (r.key)}
    <TvRailRow k={rommRowKey(r.key)} sub selected={selected === r.key} count={r.count} testId={tid(r.testId)} onSelect={() => onSelect(r.key)}>{r.label}</TvRailRow>
  {/each}
{/if}
