<script lang="ts">
  import { api, type CloudPanelInfo, type CloudRecord, type InstalledGame, type SaveType } from '../api';
  import CloudPanel from './CloudPanel.svelte';
  import {
    cloudButtonLabel,
    cloudRecordSummary,
    cloudRecordTitle,
    createRequestGuard,
    latestRecordText,
    uploadedLine,
    type CloudMode,
  } from './cloud';

  let {
    gameTitle,
    cloudGame,
    isNative,
    savePanelInfo,
    statePanelInfo,
    cloudMode,
    infoError,
    onToggle,
  }: {
    gameTitle: string;
    cloudGame: InstalledGame;
    isNative: boolean;
    savePanelInfo: CloudPanelInfo | null;
    statePanelInfo: CloudPanelInfo | null;
    cloudMode: CloudMode;
    infoError: string | null;
    onToggle: (saveType: 'save' | 'state') => void;
  } = $props();

  let activePanelInfo = $derived(
    cloudMode === 'save' ? savePanelInfo : cloudMode === 'state' ? statePanelInfo : null
  );
  let anySupported = $derived(savePanelInfo?.supported === true || statePanelInfo?.supported === true);

  // Overview cards: the newest record of each supported kind. The backend
  // returns records newest-first (`record_dtos_sort_newest_first_...`), so
  // the head of the list is the one to show. `null` = not fetched yet or
  // fetch failed; the card shows loading copy / the error respectively.
  type Latest = { record: CloudRecord | null; error: string | null } | null;
  let latest = $state<Record<SaveType, Latest>>({ save: null, state: null });
  const guards = { save: createRequestGuard(), state: createRequestGuard() };

  let supportedKinds = $derived(
    (['save', 'state'] as const).filter((kind) =>
      kind === 'save' ? savePanelInfo?.supported === true : statePanelInfo?.supported === true
    )
  );

  async function loadLatest(kind: SaveType) {
    const id = guards[kind].next();
    try {
      const records = await api.cloudRecords(cloudGame, kind);
      if (!guards[kind].isCurrent(id)) return;
      latest[kind] = { record: records[0] ?? null, error: null };
    } catch (err) {
      if (!guards[kind].isCurrent(id)) return;
      latest[kind] = { record: null, error: err instanceof Error ? err.message : String(err) };
    }
  }

  // Refetch every time the overview shows (first display, and each return
  // from Manage mode) so an upload or delete made in the panel is
  // reflected. Leaving the overview resets the cards to loading.
  $effect(() => {
    void cloudGame;
    if (cloudMode !== 'overview') return;
    for (const kind of supportedKinds) {
      latest[kind] = null;
      loadLatest(kind);
    }
  });
</script>

<div class="saves">
  {#if anySupported}
    <div class="cloud-toggle">
      {#if savePanelInfo?.supported}
        <button
          data-testid="details-cloud-save-toggle"
          class:active={cloudMode === 'save'}
          onclick={() => onToggle('save')}
        >
          {cloudButtonLabel('save', savePanelInfo.scope)}
        </button>
      {/if}
      {#if statePanelInfo?.supported}
        <button
          data-testid="details-cloud-state-toggle"
          class:active={cloudMode === 'state'}
          onclick={() => onToggle('state')}
        >
          {cloudButtonLabel('state', statePanelInfo.scope)}
        </button>
      {/if}
    </div>
  {:else}
    <p class="empty" data-testid="details-cloud-unsupported">
      {savePanelInfo?.block_reason ||
        statePanelInfo?.block_reason ||
        'Cloud saves are not configured for this game.'}
    </p>
  {/if}

  {#if infoError}
    <p data-testid="cloud-panel-info-error" class="error" role="alert">{infoError}</p>
  {/if}

  {#if cloudMode === 'overview'}
    {#each supportedKinds as kind (kind)}
      {@const text = latestRecordText(kind)}
      {@const entry = latest[kind]}
      <div data-testid={`details-latest-${kind}`} class="latest">
        <h4 class="latest-heading">{text.heading}</h4>
        {#if entry === null}
          <p class="hint">{text.loading}</p>
        {:else if entry.error}
          <p class="error" role="alert">{entry.error}</p>
        {:else if entry.record === null}
          <p class="hint">{text.empty}</p>
        {:else}
          <p class="latest-title">{cloudRecordTitle(entry.record, kind)}</p>
          <p class="hint">{cloudRecordSummary(entry.record, kind)}</p>
          <p class="hint">{uploadedLine(entry.record)}</p>
        {/if}
      </div>
    {/each}
  {:else if activePanelInfo}
    <CloudPanel
      game={cloudGame}
      {gameTitle}
      saveType={cloudMode}
      panelInfo={activePanelInfo}
      {isNative}
    />
  {/if}
</div>

<style>
  .saves {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .cloud-toggle {
    display: flex;
    gap: 8px;
  }

  .cloud-toggle button {
    flex: 1;
    font: inherit;
    padding: 8px 12px;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .cloud-toggle button.active {
    background: var(--primary);
    color: #fff;
    border-color: var(--primary);
  }

  .empty,
  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .latest {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .latest-heading {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .latest-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
</style>
