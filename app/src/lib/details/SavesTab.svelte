<script lang="ts">
  import type { CloudPanelInfo, InstalledGame, SaveType } from '../api';
  import CloudPanel from './CloudPanel.svelte';
  import { cloudKindLabel, shownCloudKind } from './cloud';

  // The tab always shows a list: the selected kind's cloud records, with
  // a Saves | States switch on top when the game syncs both. There is no
  // summary mode (user ruling 2026-10-30: a button that flipped between a
  // summary and the list read as a broken button).
  let {
    gameTitle,
    cloudGame,
    isNative,
    savePanelInfo,
    statePanelInfo,
    kind,
    infoError,
    onKind,
  }: {
    gameTitle: string;
    cloudGame: InstalledGame;
    isNative: boolean;
    savePanelInfo: CloudPanelInfo | null;
    statePanelInfo: CloudPanelInfo | null;
    /** The kind the user last picked (owned by Details.svelte). */
    kind: SaveType;
    infoError: string | null;
    onKind: (kind: SaveType) => void;
  } = $props();

  let saveSupported = $derived(savePanelInfo?.supported === true);
  let stateSupported = $derived(statePanelInfo?.supported === true);
  let shown = $derived(shownCloudKind(kind, saveSupported, stateSupported));
  let shownInfo = $derived(shown === 'save' ? savePanelInfo : shown === 'state' ? statePanelInfo : null);
</script>

<div class="saves">
  {#if saveSupported && stateSupported && savePanelInfo && statePanelInfo}
    <div class="switch" role="tablist" aria-label="Cloud saves or states">
      <button
        role="tab"
        data-testid="details-cloud-kind-save"
        aria-selected={shown === 'save'}
        class:active={shown === 'save'}
        onclick={() => onKind('save')}
      >
        {cloudKindLabel('save', savePanelInfo.scope)}
      </button>
      <button
        role="tab"
        data-testid="details-cloud-kind-state"
        aria-selected={shown === 'state'}
        class:active={shown === 'state'}
        onclick={() => onKind('state')}
      >
        {cloudKindLabel('state', statePanelInfo.scope)}
      </button>
    </div>
  {/if}

  {#if infoError}
    <p data-testid="cloud-panel-info-error" class="error" role="alert">{infoError}</p>
  {/if}

  {#if shown && shownInfo}
    {#key shown}
      <CloudPanel game={cloudGame} {gameTitle} saveType={shown} panelInfo={shownInfo} {isNative} />
    {/key}
  {:else}
    <p class="empty" data-testid="details-cloud-unsupported">
      {savePanelInfo?.block_reason ||
        statePanelInfo?.block_reason ||
        'Cloud saves are not configured for this game.'}
    </p>
  {/if}
</div>

<style>
  .saves {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  /* A segmented control: one side is always selected, so it reads as a
     choice of view rather than a button that does something. */
  .switch {
    align-self: flex-start;
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: var(--r-pill);
    background: var(--surface);
    border: 1px solid var(--line);
  }

  .switch button {
    font: inherit;
    font-size: 13px;
    padding: 5px 16px;
    border-radius: var(--r-pill);
    border: none;
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  /* `--surface` over the track's own `--surface` doubles it, so the side
     under the pointer reads in both themes. */
  .switch button:not(.active):hover {
    background: var(--surface);
  }

  .switch button.active {
    background: var(--primary);
    color: var(--on-primary);
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
