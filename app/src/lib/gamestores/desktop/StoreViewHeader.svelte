<script lang="ts">
  // A store view's title row in the Library (GameStores README, "Header";
  // prototype `stRenderView`'s title, count, chips and actions): "GOG ·
  // Installed", the count line, the chips (account, Login expired, Cloud
  // saves: on, gogdl not installed), then Backup all N…, Install from
  // installer… and Refresh on the right.
  import type { StoreGame, StoreStatus } from '../../api';
  import { navigation } from '../../stores/navigation.svelte';
  import type { StorePart } from '../rail';
  import { storeChips, storeCountLine, storeHeaderActions, storeViewTitle } from '../view';
  import { openInstaller, refreshStore, signIn } from './ui.svelte';

  let {
    status,
    part,
    games,
    installedCount,
    now,
    onBackupAll,
  }: {
    status: StoreStatus;
    part: StorePart;
    /** The part's whole list (before the search), for the count line. */
    games: readonly StoreGame[];
    installedCount: number;
    now: number;
    onBackupAll: () => void;
  } = $props();

  let title = $derived(storeViewTitle(status.source, part));
  let chips = $derived(storeChips(status));
  let actions = $derived(storeHeaderActions(status, part, installedCount));

  function chipAct(act: 'settings' | 'sign_in' | null) {
    if (act === 'settings') navigation.goSettings('stores');
    else if (act === 'sign_in') void signIn(status.source);
  }
</script>

<header data-testid="library-header" class="title-row">
  <h2 data-testid="library-title">{title.title} <span class="sub">· {title.sub}</span></h2>
  <p data-testid="library-count" class="counts">{storeCountLine(status, part, games, now)}</p>
  <div class="chips" data-testid="store-header-chips">
    {#each chips as c (c.testId)}
      {#if c.act}
        <button class="chip link" class:warn={c.warn} data-testid={c.testId} onclick={() => chipAct(c.act)}>{c.text}</button>
      {:else}
        <span class="chip" data-testid={c.testId}>{c.text}</span>
      {/if}
    {/each}
    {#if actions.backupAll > 0}
      <button class="small-btn" data-testid="library-store-backup-all" onclick={onBackupAll}>Backup all {actions.backupAll}…</button>
    {/if}
    {#if actions.installer}
      <button class="small-btn" data-testid="library-store-installer" onclick={openInstaller}>Install from installer…</button>
    {/if}
    {#if actions.refresh}
      <button class="small-btn" data-testid="library-store-refresh" onclick={() => refreshStore(status.source)}>Refresh</button>
    {/if}
  </div>
</header>

<style>
  .title-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-h);
  }

  .sub {
    color: var(--text-muted);
    font-weight: 400;
  }

  .counts {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .chip.link {
    cursor: pointer;
  }

  .chip.link:hover {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .chip.warn {
    border-color: var(--warning);
    color: var(--warning-text);
  }

  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn:hover {
    background: var(--surface);
  }
</style>
