<script lang="ts">
  import { api, type CloudPanelInfo, type DownloadStatus, type GameSummary } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { isInstalled, installed, matchesInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { sessions } from './stores/sessions.svelte';
  import Cover from './Cover.svelte';
  import CloudPanel from './details/CloudPanel.svelte';
  import { cloudButtonLabel, isNativeExecutablePlatform, syntheticCloudGame, toggleCloudMode, type CloudMode } from './details/cloud';

  let {
    game,
    platformName,
    onClose,
    onLibraryPathUnset,
  }: {
    game: GameSummary;
    platformName: string;
    onClose: () => void;
    onLibraryPathUnset: () => void;
  } = $props();

  const LIVE_INSTALL_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

  type PendingAction = 'install' | 'uninstall' | 'play' | 'stop' | null;

  let confirmingUninstall = $state(false);
  let pendingAction = $state<PendingAction>(null);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  let pending = $derived(pendingAction !== null);
  let liveEntry = $derived(
    downloads.entries.find((e) => e.rom_id === game.id && LIVE_INSTALL_STATUSES.includes(e.status))
  );
  let installedNow = $derived(isInstalled(game, platformName));
  let liveSession = $derived(sessions.sessionFor(game.id));

  // Cloud saves/states (task-19-brief.md). `cloudGame` is the InstalledGame
  // registry row when one exists, else a synthetic stand-in built from the
  // GameSummary — the cloud commands resolve "installed" themselves by
  // identity match (cloud_service.rs's `panel_info`), so a non-installed
  // shared-scope game (e.g. an entry on the synthetic `Emulators` platform)
  // can still open its panel.
  let installedRow = $derived(installed.list.find((row) => matchesInstalled(row, game, platformName)) ?? null);
  let cloudGame = $derived(installedRow ?? syntheticCloudGame(game, platformName));
  let isNative = $derived(isNativeExecutablePlatform(platformName));

  let cloudMode = $state<CloudMode>('overview');
  let savePanelInfo = $state<CloudPanelInfo | null>(null);
  let statePanelInfo = $state<CloudPanelInfo | null>(null);
  let cloudPanelInfoError = $state<string | null>(null);

  let activeCloudPanelInfo = $derived(cloudMode === 'save' ? savePanelInfo : cloudMode === 'state' ? statePanelInfo : null);

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    api
      .cloudPanelInfo(cloudGame, 'save')
      .then((info) => (savePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
    api
      .cloudPanelInfo(cloudGame, 'state')
      .then((info) => (statePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
  });

  function handleCloudToggle(saveType: 'save' | 'state') {
    cloudMode = toggleCloudMode(cloudMode, saveType);
  }

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function handleInstall() {
    error = null;
    pendingAction = 'install';
    try {
      await api.installGame(game.id);
    } catch (err) {
      const message = errorMessage(err);
      error = message;
      if (message.includes('library folder')) onLibraryPathUnset();
    } finally {
      pendingAction = null;
    }
  }

  async function handleUninstallClick() {
    if (!confirmingUninstall) {
      confirmingUninstall = true;
      return;
    }
    error = null;
    pendingAction = 'uninstall';
    try {
      await api.uninstallGame(game.id);
      await refreshInstalled();
      onClose();
    } catch (err) {
      error = errorMessage(err);
      confirmingUninstall = false;
    } finally {
      pendingAction = null;
    }
  }

  async function handlePlay() {
    error = null;
    pendingAction = 'play';
    try {
      await api.launchGame(game.id);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  async function handleStop() {
    if (!liveSession) return;
    error = null;
    pendingAction = 'stop';
    try {
      await api.stopGame(liveSession.id);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="details-panel"
    class="panel"
    class:wide={cloudMode !== 'overview'}
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={game.name}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="details-close" class="close" onclick={onClose} aria-label="Close">×</button>
    <div class="cover">
      <Cover {game} />
    </div>
    <h2>{game.name}</h2>
    {#if liveSession}
      <span data-testid="details-playing-chip" class="chip">Playing</span>
    {/if}
    <p class="platform">{platformName}</p>

    <div class="action">
      {#if liveEntry}
        <button disabled>Installing…</button>
      {:else if liveSession}
        <button data-testid="details-stop" disabled={pending} onclick={handleStop}>
          {pendingAction === 'stop' ? 'Stopping…' : 'Stop'}
        </button>
      {:else if installedNow}
        <button data-testid="details-play" disabled={pending} onclick={handlePlay}>
          {pendingAction === 'play' ? 'Launching…' : 'Play'}
        </button>
        <button
          data-testid="details-uninstall"
          class="secondary"
          class:confirm={confirmingUninstall}
          disabled={pending}
          onclick={handleUninstallClick}
        >
          {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
        </button>
      {:else}
        <button data-testid="details-install" disabled={pending} onclick={handleInstall}>
          {pendingAction === 'install' ? 'Installing…' : 'Install'}
        </button>
      {/if}
    </div>

    {#if savePanelInfo?.supported || statePanelInfo?.supported}
      <div class="cloud-toggle">
        {#if savePanelInfo?.supported}
          <button
            data-testid="details-cloud-save-toggle"
            class:active={cloudMode === 'save'}
            onclick={() => handleCloudToggle('save')}
          >
            {cloudButtonLabel('save', savePanelInfo.scope)}
          </button>
        {/if}
        {#if statePanelInfo?.supported}
          <button
            data-testid="details-cloud-state-toggle"
            class:active={cloudMode === 'state'}
            onclick={() => handleCloudToggle('state')}
          >
            {cloudButtonLabel('state', statePanelInfo.scope)}
          </button>
        {/if}
      </div>
    {/if}

    {#if cloudPanelInfoError}
      <p data-testid="cloud-panel-info-error" class="error" role="alert">{cloudPanelInfoError}</p>
    {/if}

    {#if cloudMode !== 'overview' && activeCloudPanelInfo}
      <CloudPanel
        game={cloudGame}
        gameTitle={game.name}
        saveType={cloudMode}
        panelInfo={activeCloudPanelInfo}
        {isNative}
        onBack={() => (cloudMode = 'overview')}
      />
    {/if}

    {#if error}
      <p data-testid="details-error" class="error" role="alert">{error}</p>
    {/if}
    {#if sessions.lastWarning}
      <p data-testid="details-warning" class="error warning" role="alert">
        {sessions.lastWarning}
        <button data-testid="details-warning-dismiss" class="dismiss" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">×</button>
      </p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(360px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    transition: width 0.15s ease;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .panel.wide {
    width: min(480px, calc(100vw - 48px));
    align-items: stretch;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .close:hover,
  .close:focus-visible {
    background: var(--border);
  }

  .cover {
    width: 160px;
    aspect-ratio: 3 / 4;
    border-radius: 8px;
    overflow: hidden;
  }

  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  h2 {
    margin: 4px 0 0;
    text-align: center;
    color: var(--text-h);
    font-size: 18px;
  }

  .chip {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
  }

  .platform {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .action {
    margin-top: 8px;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .action button {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .action button.confirm {
    background: #e5484d;
  }

  .action button.secondary {
    padding: 8px 16px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  .action button.secondary.confirm {
    background: transparent;
    color: #e5484d;
    border-color: #e5484d;
  }

  .cloud-toggle {
    margin-top: 4px;
    width: 100%;
    display: flex;
    gap: 8px;
  }

  .cloud-toggle button {
    flex: 1;
    font: inherit;
    padding: 8px 12px;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    cursor: pointer;
  }

  .cloud-toggle button.active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 13px;
    text-align: center;
  }

  .error.warning {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .dismiss {
    flex: none;
    width: 18px;
    height: 18px;
    line-height: 1;
    padding: 0;
    font-size: 14px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: #e5484d;
    cursor: pointer;
  }

  .dismiss:hover,
  .dismiss:focus-visible {
    background: var(--border);
  }
</style>
