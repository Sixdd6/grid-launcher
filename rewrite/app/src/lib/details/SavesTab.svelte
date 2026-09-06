<script lang="ts">
  import type { CloudPanelInfo, InstalledGame } from '../api';
  import CloudPanel from './CloudPanel.svelte';
  import { cloudButtonLabel, type CloudMode } from './cloud';

  let {
    gameTitle,
    cloudGame,
    isNative,
    savePanelInfo,
    statePanelInfo,
    cloudMode,
    infoError,
    onToggle,
    onBack,
  }: {
    gameTitle: string;
    cloudGame: InstalledGame;
    isNative: boolean;
    savePanelInfo: CloudPanelInfo | null;
    statePanelInfo: CloudPanelInfo | null;
    cloudMode: CloudMode;
    infoError: string | null;
    onToggle: (saveType: 'save' | 'state') => void;
    onBack: () => void;
  } = $props();

  let activePanelInfo = $derived(
    cloudMode === 'save' ? savePanelInfo : cloudMode === 'state' ? statePanelInfo : null
  );
  let anySupported = $derived(savePanelInfo?.supported === true || statePanelInfo?.supported === true);
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

  {#if cloudMode !== 'overview' && activePanelInfo}
    <CloudPanel
      game={cloudGame}
      {gameTitle}
      saveType={cloudMode}
      panelInfo={activePanelInfo}
      {isNative}
      {onBack}
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

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
</style>
