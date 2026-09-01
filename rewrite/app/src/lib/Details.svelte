<script lang="ts">
  import { api, type DownloadStatus, type GameSummary } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { isInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import Cover from './Cover.svelte';

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

  let confirmingUninstall = $state(false);
  let pending = $state(false);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  let liveEntry = $derived(
    downloads.entries.find((e) => e.rom_id === game.id && LIVE_INSTALL_STATUSES.includes(e.status))
  );
  let installedNow = $derived(isInstalled(game, platformName));

  $effect(() => {
    panelEl?.focus();
  });

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function handleInstall() {
    error = null;
    pending = true;
    try {
      await api.installGame(game.id);
    } catch (err) {
      const message = errorMessage(err);
      error = message;
      if (message.includes('library folder')) onLibraryPathUnset();
    } finally {
      pending = false;
    }
  }

  async function handleUninstallClick() {
    if (!confirmingUninstall) {
      confirmingUninstall = true;
      return;
    }
    error = null;
    pending = true;
    try {
      await api.uninstallGame(game.id);
      await refreshInstalled();
      onClose();
    } catch (err) {
      error = errorMessage(err);
      confirmingUninstall = false;
    } finally {
      pending = false;
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
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={game.name}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button class="close" onclick={onClose} aria-label="Close">×</button>
    <div class="cover">
      <Cover {game} />
    </div>
    <h2>{game.name}</h2>
    <p class="platform">{platformName}</p>

    <div class="action">
      {#if liveEntry}
        <button disabled>Installing…</button>
      {:else if installedNow}
        <button class:confirm={confirmingUninstall} disabled={pending} onclick={handleUninstallClick}>
          {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
        </button>
      {:else}
        <button disabled={pending} onclick={handleInstall}>{pending ? 'Installing…' : 'Install'}</button>
      {/if}
    </div>

    {#if error}
      <p class="error" role="alert">{error}</p>
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

  .platform {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .action {
    margin-top: 8px;
    width: 100%;
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
</style>
