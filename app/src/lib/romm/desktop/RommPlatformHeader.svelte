<script lang="ts">
  // A RomM platform screen's title row (RommStorefront README "RomM ›
  // Discover and platform screens"): the old Server platform header — name,
  // counts ("4 games on RomM · 1 installed · plus 1 only on this computer"),
  // the Firmware and Emulator chips that open Settings › Systems, and
  // Backup all N…. The test ids stay the Server header's.
  import { emulatorChipLabel, firmwareChipLabel, type FirmwareChipState } from '../../server/header';

  let {
    name,
    counts,
    chipsShown,
    firmware,
    firmwareShown,
    firmwareLinks,
    defaultEmulator,
    backupCount,
    onBackupAll,
    onOpenSystems,
  }: {
    name: string;
    /** The counts line; `null` while the platform's games load. */
    counts: string | null;
    /** Native and Windows platforms, and the Emulators shelf, show neither chip. */
    chipsShown: boolean;
    firmware: FirmwareChipState;
    /** The firmware chip needs a server platform (an installed-only platform has none). */
    firmwareShown: boolean;
    firmwareLinks: boolean;
    defaultEmulator: string;
    /** "Backup all N…": the server's games of the platform; 0 hides it. */
    backupCount: number;
    onBackupAll: () => void;
    onOpenSystems: () => void;
  } = $props();
</script>

<header data-testid="server-platform-header" class="platform-header">
  <h2>{name}</h2>
  {#if counts !== null}
    <p data-testid="server-platform-counts" class="counts">{counts}</p>
  {/if}
  {#if chipsShown || backupCount > 0}
    <div class="chips">
      {#if chipsShown}
        {#if firmwareShown}
          {#if firmwareLinks}
            <button
              data-testid="server-firmware-chip"
              class="chip link"
              title="Install firmware in Settings › Systems"
              onclick={onOpenSystems}
            >
              {firmwareChipLabel(firmware)}
            </button>
          {:else}
            <span data-testid="server-firmware-chip" class="chip">{firmwareChipLabel(firmware)}</span>
          {/if}
        {/if}
        <button data-testid="server-emulator-chip" class="chip link" onclick={onOpenSystems}>
          {emulatorChipLabel(defaultEmulator)}
        </button>
      {/if}
      {#if backupCount > 0}
        <!-- GameBackups README: "Backup all 11…" on the right of the header. -->
        <button data-testid="server-backup-platform" class="chip link backup-all" onclick={onBackupAll}>
          Backup all {backupCount}…
        </button>
      {/if}
    </div>
  {/if}
</header>

<style>
  .platform-header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
  }

  .platform-header h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-h);
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
    gap: 8px;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-muted);
  }

  .chip.link {
    cursor: pointer;
  }

  .chip.link:hover {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .backup-all {
    white-space: nowrap;
  }
</style>
