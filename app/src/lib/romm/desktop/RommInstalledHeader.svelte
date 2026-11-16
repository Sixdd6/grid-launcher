<script lang="ts">
  // RomM › Installed's title row (RommStorefront README "RomM › Installed";
  // prototype `rmRenderInstalled`): "RomM · Installed", "11 games · 21.2 GB ·
  // from romm.home.lan", the state chip (Signed in as …, Connecting to …,
  // Not connected · Connect…) and Backup all N…. Can't reach and Sign-in
  // expired have no chip: the banner under the header carries them.
  import type { RommStatus } from '../../api';
  import { sizeText } from '../../backups/dialog';
  import { navigation } from '../../stores/navigation.svelte';
  import { rommHost, rommInstalledChip, rommInstalledLine, rommState } from '../words';

  let {
    status,
    count,
    bytes,
    backupCount,
    backupDisabled,
    backupReason,
    onBackupAll,
  }: {
    status: RommStatus;
    /** Installed RomM games (all of them, before the search). */
    count: number;
    /** Their size on disk. */
    bytes: number;
    /** "Backup all N…": every installed RomM game (unlinked ones are listed by the dialog). */
    backupCount: number;
    backupDisabled: boolean;
    backupReason: string;
    onBackupAll: () => void;
  } = $props();

  let chip = $derived(rommInstalledChip(status));
  let connecting = $derived(rommState(status) === 'connecting');
  let line = $derived(rommInstalledLine(count, count > 0 ? sizeText(bytes) : '', rommHost(status)));
</script>

<header data-testid="library-header" class="title-row">
  <h2 data-testid="library-title">RomM <span class="sub">· Installed</span></h2>
  <p data-testid="library-count" class="counts">{line}</p>
  <div class="chips">
    {#if chip !== null}
      {#if connecting}
        <span data-testid="romm-installed-chip" class="chip"><span class="spin" aria-hidden="true"></span>{chip}</span>
      {:else}
        <!-- Signed in as … / Not connected · Connect…: the RomM card. -->
        <button data-testid="romm-installed-chip" class="chip link" onclick={() => navigation.goSettings('stores')}>
          {chip}
        </button>
      {/if}
    {/if}
    {#if backupCount > 0}
      <button
        data-testid="library-romm-backup-all"
        class="chip link backup-all"
        disabled={backupDisabled}
        title={backupReason || undefined}
        onclick={onBackupAll}
      >
        Backup all {backupCount}…
      </button>
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
    gap: 6px;
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

  .chip.link:hover:not(:disabled) {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .chip.link:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .spin {
    display: inline-block;
    width: 8px;
    height: 8px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
</style>
