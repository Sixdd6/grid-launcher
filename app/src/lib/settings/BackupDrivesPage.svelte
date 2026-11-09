<script lang="ts">
  // Settings › Backup drives (GameBackups README, "Settings › Backup
  // drives"; plan Task 16). The connected drives first, then the set-up
  // drives GRID remembers that are not connected, each as a DriveCard. The
  // list is the drives store's live snapshot, so a drive appears and goes
  // within a scan of being plugged in or removed. Below: the one backup
  // default (firmware) and the gridpack layout.
  import { api } from '../api';
  import DriveCard from '../backups/DriveCard.svelte';
  import { hostPackOs } from '../backups/drives';
  import { drives } from '../stores/drives.svelte';

  let {
    active = true,
    onShowInLibrary = undefined,
  }: {
    active?: boolean;
    /** Opens Library on a drive (Shell). */
    onShowInLibrary?: (key: string) => void;
  } = $props();

  const hostOs = hostPackOs(navigator.platform);

  let now = $state(new Date());
  let includeFirmware = $state<boolean | null>(null);
  let savingFirmware = $state(false);
  let firmwareError = $state<string | null>(null);

  function errorText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  // Re-read the default and the clock ("Last seen 3 days ago") each time
  // the page comes forward.
  $effect(() => {
    if (!active) return;
    now = new Date();
    api
      .getBackupSettings()
      .then((s) => {
        includeFirmware = s.include_firmware;
        firmwareError = null;
      })
      .catch((err) => {
        firmwareError = errorText(err);
      });
  });

  async function setFirmware(value: boolean) {
    const before = includeFirmware;
    includeFirmware = value;
    savingFirmware = true;
    firmwareError = null;
    try {
      await api.setBackupIncludeFirmware(value);
    } catch (err) {
      includeFirmware = before;
      firmwareError = errorText(err);
    } finally {
      savingFirmware = false;
    }
  }
</script>

<div data-testid="settings-backup-drives-pane" class="backups">
  <p class="g-note lede">
    Copy games from your server to a USB drive, with their art, details, emulators and firmware. Plug the drive into
    any computer with GRID to install them, even without a network.
  </p>

  <section class="g-sub" aria-labelledby="backup-connected-title">
    <h3 id="backup-connected-title">Connected</h3>
    {#if !drives.loaded}
      <p class="g-note">Looking for drives…</p>
    {:else if drives.connected.length === 0}
      <div class="empty" data-testid="backup-drives-empty">
        <b>No removable drives</b>
        <span class="g-note">Plug in a USB flash drive or SSD. It shows up here.</span>
      </div>
    {:else}
      <div class="cards">
        {#each drives.connected as d (d.key)}
          <DriveCard drive={d} {hostOs} {now} {onShowInLibrary} />
        {/each}
      </div>
    {/if}
  </section>

  {#if drives.remembered.length > 0}
    <section class="g-sub" aria-labelledby="backup-away-title">
      <h3 id="backup-away-title">Not connected</h3>
      <div class="cards">
        {#each drives.remembered as r (r.mount)}
          <DriveCard remembered={r} {now} />
        {/each}
      </div>
    </section>
  {/if}

  <section class="g-sub" aria-labelledby="backup-defaults-title">
    <h3 id="backup-defaults-title">Every backup also copies</h3>
    <label class="g-check">
      <input
        type="checkbox"
        data-testid="backup-include-firmware"
        checked={includeFirmware === true}
        disabled={includeFirmware === null || savingFirmware}
        onchange={(e) => setFirmware(e.currentTarget.checked)}
      />
      The server's firmware for the systems you backup
    </label>
    <p class="g-note">
      You can change this for each backup. Each game always takes its covers, fanart, screenshots, logo and details, and
      the emulator its system needs, built for every computer the drive is for. Windows games take Proton when the drive
      is for Linux.
    </p>
    {#if firmwareError}<p class="g-error" role="alert">{firmwareError}</p>{/if}
  </section>

  <details class="tree">
    <summary>What's in the gridpack folder</summary>
    <pre>{`gridpack/
  gridpack.json          pack name, computers, format version, game index
  games/<platform>/      the files the server gives (zip, 7z, iso, chd, folder)
  assets/<platform>/<game>/
    cover.jpg  fanart.jpg  logo.png  screenshots/
  emulators/<emulator>/<version>/<os>/
  firmware/<platform>/`}</pre>
  </details>
</div>

<style>
  .backups {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .lede {
    max-width: 620px;
  }

  .cards {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* No drives: a dashed box, as the prototype's `.empty-state`. */
  .empty {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px;
    border-radius: var(--r-row);
    border: 1px dashed var(--line-strong);
  }

  .empty b {
    color: var(--text-h);
    font-size: 14px;
  }

  .tree summary {
    cursor: pointer;
    color: var(--text-h);
    font-size: 14px;
    font-weight: 600;
  }

  .tree pre {
    margin: 8px 0 0;
    padding: 10px 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text-h);
    font:
      12px/1.6 ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
    overflow-x: auto;
    user-select: text;
  }
</style>
