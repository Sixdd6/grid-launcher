<script lang="ts">
  // Details › Saves (items 306–314; tvmode.js `detBody`, `savesOf`): an
  // unlinked game, or one never installed or played, says so (item 314);
  // otherwise the panel for the kind chosen with the Saves | States switch.
  import type { DetailsStore } from '../../../details/detailsStore.svelte';
  import { savesNote } from '../details';
  import { det } from '../detailsState.svelte';
  import TvSavesPanel from './TvSavesPanel.svelte';

  let {
    d,
    installed,
    unlinked,
    noId,
    lastPlayedAt,
  }: { d: DetailsStore; installed: boolean; unlinked: boolean; noId: boolean; lastPlayedAt: number } = $props();

  let note = $derived(savesNote({ unlinked, noId, installed, lastPlayedAt }));
  let info = $derived(det.saveKind === 'save' ? d.savePanelInfo : d.statePanelInfo);
</script>

{#if note}
  <p class="note" data-testid="tv-saves-note">{note}</p>
{:else}
  {#key `${det.saveKind}:${info === null}`}
    <TvSavesPanel {d} {info} kind={det.saveKind} />
  {/key}
  {#if d.cloudPanelInfoError}
    <p class="note err" data-testid="tv-saves-error">{d.cloudPanelInfoError}</p>
  {/if}
{/if}
